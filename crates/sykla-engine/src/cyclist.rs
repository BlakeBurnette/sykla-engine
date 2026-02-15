use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};

use crate::gltf;
use crate::mesh::CpuMesh;
use crate::terrain::{RoutePoint, interpolate_point, offset_position};

// ── Cyclist instance data (32 bytes, vertex locations 3-7) ─────

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CyclistInstanceData {
    pub position: [f32; 3],
    pub scale: f32,
    pub forward: [f32; 2],
    pub pedal_phase: f32,
    pub _pad: f32,
}

impl CyclistInstanceData {
    const ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        3 => Float32x3,
        4 => Float32,
        5 => Float32x2,
        6 => Float32,
        7 => Float32,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CyclistInstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRS,
        }
    }
}

// ── Multi-part cyclist mesh (facing +Z) ─────────────────────────

pub struct CyclistPart {
    pub mesh: CpuMesh,
    pub default_color: [f32; 4],
}

/// Load cyclist model from a GLB file. Each primitive becomes a CyclistPart
/// with its material base_color. UV.x bone encoding is baked into the mesh.
///
/// The model was exported facing -Z; we flip Z here so the engine's +Z forward
/// convention is satisfied, then reverse triangle winding to correct the reflection.
pub fn load_cyclist_glb(data: &[u8]) -> Vec<CyclistPart> {
    let glb = gltf::parse_glb(data).expect("failed to parse cyclist GLB");

    // Bike geometry constants (must match skeleton.rs)
    const CRANKSET_BONE: u32 = 21;
    const CRANK_R: f32 = 0.124;

    glb.primitives
        .into_iter()
        .map(|prim| {
            let mut mesh = prim.mesh;
            // Flip Z: model faces -Z but engine expects +Z forward
            for v in &mut mesh.vertices {
                v.position[2] = -v.position[2];
                v.normal[2] = -v.normal[2];
            }
            // Fix winding order (Z-flip is a reflection)
            for tri in mesh.indices.chunks_exact_mut(3) {
                tri.swap(1, 2);
            }

            // Fix pedal positions: the bike model has pedal mesh at the BB center
            // (Y≈0.306) instead of at the crank arm tips. Only apply to primitives
            // where bone-21 vertices are clustered near BB (pedal mesh, Y extent < 5cm),
            // NOT to primitives where they span the full arm (crank arm, Y extent ~25cm).
            let (mut b21_min_y, mut b21_max_y) = (f32::MAX, f32::MIN);
            for v in &mesh.vertices {
                if v.uv[0].round() as u32 == CRANKSET_BONE {
                    b21_min_y = b21_min_y.min(v.position[1]);
                    b21_max_y = b21_max_y.max(v.position[1]);
                }
            }
            let is_pedal_mesh = b21_max_y > b21_min_y && (b21_max_y - b21_min_y) < 0.05;
            if is_pedal_mesh {
                for v in &mut mesh.vertices {
                    if v.uv[0].round() as u32 == CRANKSET_BONE {
                        if v.position[0] < -0.01 {
                            // Right pedal → shift to top arm tip (θ=0 = TDC for right)
                            v.position[1] += CRANK_R;
                        } else if v.position[0] > 0.01 {
                            // Left pedal → shift to bottom arm tip (opposite of right)
                            v.position[1] -= CRANK_R;
                        }
                    }
                }
            }

            CyclistPart { mesh, default_color: prim.base_color }
        })
        .collect()
}

// ── NPC cyclist state ──────────────────────────────────────────

pub struct NpcCyclist {
    pub distance: f32,
    pub lateral_offset: f32,
    pub speed: f32,
}

pub fn spawn_npc_cyclists(route_length: f32, count: usize) -> Vec<NpcCyclist> {
    let mut rng: u64 = 54321;
    let mut npcs = Vec::with_capacity(count);
    let spacing = route_length / count.max(1) as f32;

    for i in 0..count {
        let base_dist = i as f32 * spacing;
        let jitter = (next_rng(&mut rng) - 0.5) * spacing * 0.5;
        let distance = (base_dist + jitter).rem_euclid(route_length);
        let lateral_offset = (next_rng(&mut rng) - 0.5) * 3.0;
        let speed = 6.0 + next_rng(&mut rng) * 4.0;

        npcs.push(NpcCyclist {
            distance,
            lateral_offset,
            speed,
        });
    }

    npcs
}

/// Road surface offset — the road mesh sits 0.08m above raw terrain elevation.
const ROAD_SURFACE_OFFSET: f32 = 0.08;

pub fn update_cyclists(
    npcs: &mut [NpcCyclist],
    route_points: &[RoutePoint],
    route_length: f32,
    dt: f32,
) -> Vec<CyclistInstanceData> {
    if route_length <= 0.0 {
        return Vec::new();
    }

    npcs.iter_mut()
        .map(|npc| {
            npc.distance = (npc.distance + npc.speed * dt) % route_length;
            let (px, pz, elev, fx, fz) = interpolate_point(route_points, npc.distance as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, npc.lateral_offset);

            // Pedal phase from distance (~80 RPM at typical speeds)
            let pedal_phase = (npc.distance / 5.3) * TAU;

            CyclistInstanceData {
                position: [wx, elev + ROAD_SURFACE_OFFSET, wz],
                scale: 1.0,
                forward: [fx, fz],
                pedal_phase,
                _pad: 0.0,
            }
        })
        .collect()
}

fn next_rng(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as u32 as f32) / (u32::MAX as f32)
}
