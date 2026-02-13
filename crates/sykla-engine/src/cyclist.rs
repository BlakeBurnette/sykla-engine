use bytemuck::{Pod, Zeroable};

use crate::mesh::CpuMesh;
use crate::mesh_helpers::{add_cylinder, add_ellipsoid};
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

// ── Procedural cyclist mesh (facing +Z) ────────────────────────

pub fn generate_cyclist() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let s = 8; // cylinder sides (smooth round tubes)

    // --- Bike frame (slightly thicker tubes for visual presence) ---
    let bb: [f32; 3] = [0.0, 0.30, 0.0];
    let seat_cluster: [f32; 3] = [0.0, 0.84, -0.07];
    let saddle: [f32; 3] = [0.0, 0.92, -0.10];
    let ht_top: [f32; 3] = [0.0, 0.86, 0.42];
    let ht_bot: [f32; 3] = [0.0, 0.52, 0.44];

    // Seat tube
    add_cylinder(&mut vertices, &mut indices, bb, 0.020, seat_cluster, 0.018, s);
    // Seatpost (thinner, visible above seat tube)
    add_cylinder(&mut vertices, &mut indices, seat_cluster, 0.014, saddle, 0.012, s);
    // Top tube
    add_cylinder(&mut vertices, &mut indices, seat_cluster, 0.018, ht_top, 0.018, s);
    // Down tube
    add_cylinder(&mut vertices, &mut indices, ht_bot, 0.020, bb, 0.020, s);
    // Head tube
    add_cylinder(&mut vertices, &mut indices, ht_bot, 0.022, ht_top, 0.022, s);
    // Chain stays
    add_cylinder(&mut vertices, &mut indices, bb, 0.014, [0.03, 0.34, -0.44], 0.010, s);
    add_cylinder(&mut vertices, &mut indices, bb, 0.014, [-0.03, 0.34, -0.44], 0.010, s);
    // Seat stays
    add_cylinder(&mut vertices, &mut indices, [0.03, 0.82, -0.07], 0.010, [0.03, 0.34, -0.44], 0.008, s);
    add_cylinder(&mut vertices, &mut indices, [-0.03, 0.82, -0.07], 0.010, [-0.03, 0.34, -0.44], 0.008, s);
    // Fork
    add_cylinder(&mut vertices, &mut indices, ht_bot, 0.016, [0.0, 0.34, 0.50], 0.012, s);

    // --- Wheels (higher-res ellipsoids) ---
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.34, 0.50, 0.02, 0.34, 0.34, 6, 12);
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.34, -0.44, 0.02, 0.34, 0.34, 6, 12);

    // --- Stem + drop handlebars ---
    let stem_end: [f32; 3] = [0.0, 0.88, 0.46];
    add_cylinder(&mut vertices, &mut indices, ht_top, 0.014, stem_end, 0.014, s);

    // Bar top (straight across)
    let bar_l: [f32; 3] = [-0.18, 0.88, 0.46];
    let bar_r: [f32; 3] = [0.18, 0.88, 0.46];
    add_cylinder(&mut vertices, &mut indices, bar_l, 0.010, bar_r, 0.010, s);

    // Right drop (3 segments: forward, down, hook back)
    let drop_r1: [f32; 3] = [0.18, 0.84, 0.49];
    let drop_r2: [f32; 3] = [0.18, 0.78, 0.50];
    let drop_r3: [f32; 3] = [0.18, 0.74, 0.46];
    add_cylinder(&mut vertices, &mut indices, bar_r, 0.010, drop_r1, 0.010, s);
    add_cylinder(&mut vertices, &mut indices, drop_r1, 0.010, drop_r2, 0.010, s);
    add_cylinder(&mut vertices, &mut indices, drop_r2, 0.010, drop_r3, 0.010, s);

    // Left drop
    let drop_l1: [f32; 3] = [-0.18, 0.84, 0.49];
    let drop_l2: [f32; 3] = [-0.18, 0.78, 0.50];
    let drop_l3: [f32; 3] = [-0.18, 0.74, 0.46];
    add_cylinder(&mut vertices, &mut indices, bar_l, 0.010, drop_l1, 0.010, s);
    add_cylinder(&mut vertices, &mut indices, drop_l1, 0.010, drop_l2, 0.010, s);
    add_cylinder(&mut vertices, &mut indices, drop_l2, 0.010, drop_l3, 0.010, s);

    // --- Saddle ---
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.94, -0.10, 0.06, 0.02, 0.12, 5, 10);

    // --- Cranks ---
    add_cylinder(&mut vertices, &mut indices, bb, 0.010, [0.08, 0.22, 0.0], 0.008, s);
    add_cylinder(&mut vertices, &mut indices, bb, 0.010, [-0.08, 0.38, 0.0], 0.008, s);

    // ==========================================================
    // Rider (aggressive road racing tuck, ~40-45° torso lean)
    // ==========================================================

    // --- Torso (smooth tapered cylinder + subtle volume) ---
    let hip_center: [f32; 3] = [0.0, 0.90, -0.06];
    let shoulder_center: [f32; 3] = [0.0, 1.26, 0.20];

    // Core torso cylinder: narrower at hips, wider at shoulders
    add_cylinder(&mut vertices, &mut indices,
        hip_center, 0.10, shoulder_center, 0.12, s);
    // Hip/pelvis width (flat, just adds lateral volume)
    add_ellipsoid(&mut vertices, &mut indices,
        0.0, 0.92, -0.05, 0.14, 0.05, 0.08, 5, 10);
    // Chest/back depth (adds volume to upper torso)
    add_ellipsoid(&mut vertices, &mut indices,
        0.0, 1.18, 0.16, 0.13, 0.08, 0.10, 5, 10);

    // --- Neck ---
    add_cylinder(&mut vertices, &mut indices,
        shoulder_center, 0.04, [0.0, 1.32, 0.24], 0.035, s);

    // --- Head + aero helmet (single elongated ellipsoid) ---
    add_ellipsoid(&mut vertices, &mut indices,
        0.0, 1.38, 0.28, 0.09, 0.10, 0.13, 6, 12);

    // --- Arms ---
    let sh_x = 0.18;
    let elbow_r: [f32; 3] = [0.20, 1.06, 0.34];
    let elbow_l: [f32; 3] = [-0.20, 1.06, 0.34];
    let hand_r: [f32; 3] = [0.20, 0.88, 0.46];
    let hand_l: [f32; 3] = [-0.20, 0.88, 0.46];

    // Upper arms
    add_cylinder(&mut vertices, &mut indices,
        [sh_x, 1.26, 0.20], 0.045, elbow_r, 0.038, s);
    add_cylinder(&mut vertices, &mut indices,
        [-sh_x, 1.26, 0.20], 0.045, elbow_l, 0.038, s);

    // Lower arms
    add_cylinder(&mut vertices, &mut indices,
        elbow_r, 0.038, hand_r, 0.030, s);
    add_cylinder(&mut vertices, &mut indices,
        elbow_l, 0.038, hand_l, 0.030, s);

    // Hands
    add_ellipsoid(&mut vertices, &mut indices,
        hand_r[0], hand_r[1], hand_r[2], 0.025, 0.020, 0.030, 5, 10);
    add_ellipsoid(&mut vertices, &mut indices,
        hand_l[0], hand_l[1], hand_l[2], 0.025, 0.020, 0.030, 5, 10);

    // --- Legs ---
    let hip_x = 0.10;
    let hip_y = 0.90;
    let hip_z: f32 = -0.06;

    // Right leg (extended, crank down)
    let knee_r: [f32; 3] = [0.10, 0.54, 0.08];
    let ankle_r: [f32; 3] = [0.08, 0.26, 0.0];
    let pedal_r: [f32; 3] = [0.08, 0.22, 0.0];

    // Left leg (bent, crank up)
    let knee_l: [f32; 3] = [-0.10, 0.62, 0.04];
    let ankle_l: [f32; 3] = [-0.08, 0.42, 0.0];
    let pedal_l: [f32; 3] = [-0.08, 0.38, 0.0];

    // Upper legs (thighs) — muscular, 0.07 at hip tapering to 0.05 at knee
    add_cylinder(&mut vertices, &mut indices,
        [hip_x, hip_y, hip_z], 0.070, knee_r, 0.050, s);
    add_cylinder(&mut vertices, &mut indices,
        [-hip_x, hip_y, hip_z], 0.070, knee_l, 0.050, s);

    // Lower legs (calves) — tapered from knee to ankle
    add_cylinder(&mut vertices, &mut indices,
        knee_r, 0.050, ankle_r, 0.025, s);
    add_cylinder(&mut vertices, &mut indices,
        knee_l, 0.050, ankle_l, 0.025, s);

    // Calf muscle bulges (ellipsoids just below knee)
    add_ellipsoid(&mut vertices, &mut indices,
        knee_r[0], knee_r[1] - 0.06, (knee_r[2] + ankle_r[2]) / 2.0,
        0.050, 0.070, 0.045, 5, 10);
    add_ellipsoid(&mut vertices, &mut indices,
        knee_l[0], knee_l[1] - 0.06, (knee_l[2] + ankle_l[2]) / 2.0,
        0.050, 0.070, 0.045, 5, 10);

    // Feet/shoes (small ellipsoids at pedal positions)
    add_ellipsoid(&mut vertices, &mut indices,
        pedal_r[0], pedal_r[1], pedal_r[2], 0.030, 0.020, 0.060, 5, 10);
    add_ellipsoid(&mut vertices, &mut indices,
        pedal_l[0], pedal_l[1], pedal_l[2], 0.030, 0.020, 0.060, 5, 10);

    CpuMesh { vertices, indices }
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

            CyclistInstanceData {
                position: [wx, elev, wz],
                scale: 1.0,
                forward: [fx, fz],
                pedal_phase: 0.0,
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
