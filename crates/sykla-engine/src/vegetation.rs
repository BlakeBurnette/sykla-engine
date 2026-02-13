use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::{RoutePoint, interpolate_point, offset_position};
use bytemuck::{Pod, Zeroable};
use std::f32::consts::{PI, TAU};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 3],
    pub scale: f32,
}

impl InstanceData {
    const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        3 => Float32x3,
        4 => Float32,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRS,
        }
    }
}

// ── Oak tree — 18m at scale 1.0 (40-80ft with scale variation) ──

pub fn generate_trunk() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let sides = 10;
    let sections = 6;

    // Main trunk: thick at base, tapers toward crown
    // 18m oak: trunk is roughly lower 40-50% before major branching
    let trunk_h = 9.0_f32;
    let base_r = 0.75_f32; // ~1.5m diameter at base
    let top_r = 0.40_f32;

    for s in 0..=sections {
        let t = s as f32 / sections as f32;
        let y = t * trunk_h;
        let r = base_r + (top_r - base_r) * t;
        // Root flare — buttressed base
        let flare = ((1.0 - t) * PI * 0.5).sin().powf(1.5) * 0.35;
        let r = r + flare;
        // Slight barrel swell
        let swell = (t * PI).sin() * 0.06;
        let r = r + swell;

        for i in 0..sides {
            let angle = (i as f32 / sides as f32) * TAU;
            let bumps = 1.0 + 0.07 * ((angle * 3.0).sin() + (angle * 5.0 + 1.0).cos());
            let rr = r * bumps;
            vertices.push(Vertex {
                position: [angle.cos() * rr, y, angle.sin() * rr],
                normal: [angle.cos(), 0.0, angle.sin()],
                uv: [i as f32 / sides as f32, t],
            });
        }
    }

    for s in 0..sections {
        for i in 0..sides {
            let bl = (s * sides + i) as u32;
            let br = (s * sides + (i + 1) % sides) as u32;
            let tl = bl + sides as u32;
            let tr = br + sides as u32;
            indices.extend_from_slice(&[bl, tl, br, br, tl, tr]);
        }
    }

    // Major branches — 5 limbs radiating from upper trunk
    let branch_data = [
        ([0.0_f32, 7.0, 0.0], [1.0_f32, 0.6, 0.3], 4.5_f32, 0.25_f32, 0.10_f32),
        ([0.0, 7.5, 0.0], [-0.9, 0.7, 0.5], 4.0, 0.22, 0.09),
        ([0.0, 6.5, 0.0], [0.3, 0.5, -1.0], 5.0, 0.28, 0.12),
        ([0.0, 8.0, 0.0], [-0.6, 0.8, -0.7], 3.5, 0.20, 0.08),
        ([0.0, 7.2, 0.0], [0.7, 0.9, 0.8], 3.8, 0.22, 0.09),
    ];

    for (base, dir, length, br, tr) in &branch_data {
        let v_base = vertices.len() as u32;
        let b_sides = 6;
        let b_sections = 3;

        let dlen = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let ax = dir[0] / dlen;
        let ay = dir[1] / dlen;
        let az = dir[2] / dlen;

        let (ux, uy, uz) = if ax.abs() < 0.9 { (1.0, 0.0, 0.0) } else { (0.0, 1.0, 0.0) };
        let rx = ay * uz - az * uy;
        let ry = az * ux - ax * uz;
        let rz = ax * uy - ay * ux;
        let rlen = (rx * rx + ry * ry + rz * rz).sqrt();
        let (rx, ry, rz) = (rx / rlen, ry / rlen, rz / rlen);
        let fx = ry * az - rz * ay;
        let fy = rz * ax - rx * az;
        let fz = rx * ay - ry * ax;

        for s in 0..=b_sections {
            let t = s as f32 / b_sections as f32;
            let r = br + (tr - br) * t;
            let cx = base[0] + ax * length * t;
            let cy = base[1] + ay * length * t;
            let cz = base[2] + az * length * t;

            for i in 0..b_sides {
                let angle = (i as f32 / b_sides as f32) * TAU;
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                vertices.push(Vertex {
                    position: [
                        cx + (rx * cos_a + fx * sin_a) * r,
                        cy + (ry * cos_a + fy * sin_a) * r,
                        cz + (rz * cos_a + fz * sin_a) * r,
                    ],
                    normal: [rx * cos_a + fx * sin_a, ry * cos_a + fy * sin_a, rz * cos_a + fz * sin_a],
                    uv: [i as f32 / b_sides as f32, t],
                });
            }
        }

        for s in 0..b_sections {
            for i in 0..b_sides {
                let bl = v_base + (s * b_sides + i) as u32;
                let br_i = v_base + (s * b_sides + (i + 1) % b_sides) as u32;
                let tl = bl + b_sides as u32;
                let tr_i = br_i + b_sides as u32;
                indices.extend_from_slice(&[bl, tl, br_i, br_i, tl, tr_i]);
            }
        }
    }

    CpuMesh { vertices, indices }
}

pub fn generate_canopy() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Wide spreading crown — oak canopy is broad and rounded
    // Total height ~18m: trunk to ~9m, canopy from ~8m to ~18m
    let lobes: &[([f32; 3], f32, f32)] = &[
        // (center, radius, y_scale)
        ([0.0, 13.0, 0.0], 5.0, 0.55),    // main central dome
        ([3.5, 12.0, 1.5], 4.0, 0.48),     // right-front
        ([-3.0, 12.5, -2.5], 3.8, 0.50),   // left-back
        ([2.5, 13.5, -3.0], 3.5, 0.46),    // right-back
        ([-3.5, 11.5, 2.0], 3.8, 0.50),    // left-front
        ([0.8, 15.0, 0.5], 3.0, 0.42),     // top crown
        ([-1.5, 12.0, 3.5], 3.2, 0.45),    // front extension
        ([1.0, 11.0, -3.5], 3.0, 0.48),    // back extension
    ];

    let stacks = 6;
    let slices = 10;

    for (center, radius, y_scale) in lobes {
        let v_base = vertices.len() as u32;

        for i in 0..=stacks {
            let phi = PI * i as f32 / stacks as f32;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            for j in 0..=slices {
                let theta = TAU * j as f32 / slices as f32;
                let lump = 1.0
                    + 0.15 * ((theta * 3.0).sin() * (phi * 2.0).cos())
                    + 0.10 * ((theta * 5.0 + 0.7).sin() * (phi * 3.0).sin());
                let r = radius * lump;
                let x = center[0] + theta.cos() * sin_phi * r;
                let y = center[1] + cos_phi * r * y_scale;
                let z = center[2] + theta.sin() * sin_phi * r;

                let nx = theta.cos() * sin_phi;
                let ny = cos_phi * y_scale;
                let nz = theta.sin() * sin_phi;
                let len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);

                vertices.push(Vertex {
                    position: [x, y, z],
                    normal: [nx / len, ny / len, nz / len],
                    uv: [j as f32 / slices as f32, i as f32 / stacks as f32],
                });
            }
        }

        for i in 0..stacks {
            for j in 0..slices {
                let a = v_base + (i * (slices + 1) + j) as u32;
                let b = a + (slices + 1) as u32;
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }

    CpuMesh { vertices, indices }
}

// ── Loblolly pine — 24m at scale 1.0 (80ft eastern NC pine) ─────

pub fn generate_pine_trunk() -> CpuMesh {
    let sides = 8;
    // Loblolly pine: long straight trunk, branches only in upper 1/3
    let height = 18.0_f32;
    let base_r = 0.35_f32; // ~0.7m diameter
    let top_r = 0.12_f32;
    let sections = 8;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for s in 0..=sections {
        let t = s as f32 / sections as f32;
        let y = t * height;
        let r = base_r + (top_r - base_r) * t;
        // Root flare
        let flare = ((1.0 - t) * PI * 0.5).sin().powf(2.0) * 0.12;
        let r = r + flare;

        for i in 0..sides {
            let angle = (i as f32 / sides as f32) * TAU;
            vertices.push(Vertex {
                position: [angle.cos() * r, y, angle.sin() * r],
                normal: [angle.cos(), 0.0, angle.sin()],
                uv: [i as f32 / sides as f32, t],
            });
        }
    }

    for s in 0..sections {
        for i in 0..sides {
            let bl = (s * sides + i) as u32;
            let br = (s * sides + (i + 1) % sides) as u32;
            let tl = bl + sides as u32;
            let tr = br + sides as u32;
            indices.extend_from_slice(&[bl, tl, br, br, tl, tr]);
        }
    }

    CpuMesh { vertices, indices }
}

pub fn generate_pine_canopy() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Loblolly pine: branches in upper 40%, wider near bottom of crown
    // Total tree ~24m. Trunk bare to ~14m, crown from 12m to 24m.
    let num_whorls = 10;
    let base_y = 12.0_f32;
    let top_y = 23.0_f32;
    let max_radius = 4.5_f32; // lower whorls
    let min_radius = 0.5_f32; // top whorl
    let slices = 10;
    let rings = 4;

    for w in 0..num_whorls {
        let wt = w as f32 / (num_whorls - 1) as f32;
        let y_center = base_y + (top_y - base_y) * wt;
        let whorl_radius = max_radius + (min_radius - max_radius) * wt;
        let angle_offset = w as f32 * 0.45;

        let v_base = vertices.len() as u32;

        // Center vertex
        vertices.push(Vertex {
            position: [0.0, y_center + 0.1, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.5, 0.5],
        });

        for r in 1..=rings {
            let rt = r as f32 / rings as f32;
            let ring_r = whorl_radius * rt;
            // Branches droop more toward tips, lower whorls droop more
            let droop_strength = 0.20 + (1.0 - wt) * 0.15;
            let droop = -rt * rt * whorl_radius * droop_strength;
            let lift = (rt * PI).sin() * 0.35;
            let y = y_center + droop + lift;

            for j in 0..=slices {
                let theta = TAU * j as f32 / slices as f32 + angle_offset;
                let noise = 1.0 + 0.12 * ((theta * 3.0 + wt * 7.0).sin())
                    + 0.08 * ((theta * 5.0 + wt * 3.0).cos());
                let rr = ring_r * noise;
                let x = theta.cos() * rr;
                let z = theta.sin() * rr;

                let nx = theta.cos() * rt * 0.3;
                let ny = 1.0 - rt * 0.5;
                let nz = theta.sin() * rt * 0.3;
                let nlen = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);

                vertices.push(Vertex {
                    position: [x, y, z],
                    normal: [nx / nlen, ny / nlen, nz / nlen],
                    uv: [j as f32 / slices as f32, rt],
                });
            }
        }

        // Center to first ring
        let center = v_base;
        for j in 0..slices {
            let a = v_base + 1 + j as u32;
            let b = a + 1;
            indices.extend_from_slice(&[center, a, b]);
        }

        // Ring-to-ring
        for r in 0..(rings - 1) {
            for j in 0..slices {
                let a = v_base + 1 + (r * (slices + 1) + j) as u32;
                let b = a + (slices + 1) as u32;
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }

        // Bottom cap
        let v_base2 = vertices.len() as u32;
        vertices.push(Vertex {
            position: [0.0, y_center - 0.08, 0.0],
            normal: [0.0, -1.0, 0.0],
            uv: [0.5, 0.5],
        });
        let outer_start = v_base + 1 + ((rings - 1) * (slices + 1)) as u32;
        for j in 0..slices {
            let a = outer_start + j as u32;
            let b = a + 1;
            indices.extend_from_slice(&[b, v_base2, a]);
        }
    }

    // Pointed leader at top
    let tip_base = vertices.len() as u32;
    let tip_y = top_y + 0.2;
    let tip_r = 0.7_f32;
    for j in 0..=slices {
        let theta = TAU * j as f32 / slices as f32;
        vertices.push(Vertex {
            position: [theta.cos() * tip_r, tip_y - 0.3, theta.sin() * tip_r],
            normal: [theta.cos() * 0.3, 0.95, theta.sin() * 0.3],
            uv: [j as f32 / slices as f32, 0.0],
        });
    }
    let tip_top = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, tip_y + 1.5, 0.0],
        normal: [0.0, 1.0, 0.0],
        uv: [0.5, 1.0],
    });
    for j in 0..slices {
        let a = tip_base + j as u32;
        let b = a + 1;
        indices.extend_from_slice(&[a, tip_top, b]);
    }

    CpuMesh { vertices, indices }
}

// ── Bush / understory shrub ─────────────────────────────────────

pub fn generate_bush() -> CpuMesh {
    let stacks = 5;
    let slices = 8;
    let radius = 1.2_f32;
    let y_center = 0.8_f32;
    let y_scale = 0.45_f32;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let phi = PI * i as f32 / stacks as f32;
        for j in 0..=slices {
            let theta = TAU * j as f32 / slices as f32;
            let lump = 1.0 + 0.25 * (theta * 2.0).sin() + 0.12 * (theta * 4.0 + 1.0).cos();
            let r = radius * lump;
            let x = theta.cos() * phi.sin() * r;
            let y = phi.cos() * r * y_scale + y_center;
            let z = theta.sin() * phi.sin() * r;
            let nx = theta.cos() * phi.sin();
            let ny = phi.cos() * y_scale;
            let nz = theta.sin() * phi.sin();
            let len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);
            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx / len, ny / len, nz / len],
                uv: [j as f32 / slices as f32, i as f32 / stacks as f32],
            });
        }
    }

    for i in 0..stacks {
        for j in 0..slices {
            let a = (i * (slices + 1) + j) as u32;
            let b = a + (slices + 1) as u32;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    CpuMesh { vertices, indices }
}

// ── Placement ─────────────────────────────────────────────────────

pub struct VegetationConfig {
    pub spacing_m: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub trees_per_slot: usize,
    pub min_scale: f32,
    pub max_scale: f32,
}

impl Default for VegetationConfig {
    fn default() -> Self {
        Self {
            spacing_m: 5.0,
            min_distance: 4.0,
            max_distance: 90.0,
            trees_per_slot: 5,
            min_scale: 0.7,
            max_scale: 1.15,
        }
    }
}

fn next_rng(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as u32 as f32) / (u32::MAX as f32)
}

/// Placement results split by tree type for separate instanced draw calls.
pub struct PlacementResult {
    pub deciduous: Vec<InstanceData>,
    pub pine: Vec<InstanceData>,
    pub bush: Vec<InstanceData>,
    pub young_deciduous: Vec<InstanceData>,
    pub young_pine: Vec<InstanceData>,
}

pub fn place_vegetation(points: &[RoutePoint], config: &VegetationConfig) -> PlacementResult {
    let mut deciduous = Vec::new();
    let mut pine = Vec::new();
    let mut bush = Vec::new();
    let mut young_deciduous = Vec::new();
    let mut young_pine = Vec::new();
    let mut rng: u64 = 42;

    let total_dist = points.last().map(|p| p.distance_m).unwrap_or(0.0);
    let num_slots = (total_dist / config.spacing_m as f64) as usize;

    for slot in 0..num_slots {
        let base_z = slot as f64 * config.spacing_m as f64;

        for _ in 0..config.trees_per_slot {
            for side in [-1.0_f32, 1.0] {
                place_one(
                    points, config, &mut rng, base_z, side,
                    &mut deciduous, &mut pine, &mut bush,
                    &mut young_deciduous, &mut young_pine,
                );
            }
        }
    }

    PlacementResult { deciduous, pine, bush, young_deciduous, young_pine }
}

fn place_one(
    points: &[RoutePoint],
    config: &VegetationConfig,
    rng: &mut u64,
    base_z: f64,
    side: f32,
    deciduous: &mut Vec<InstanceData>,
    pine: &mut Vec<InstanceData>,
    bush: &mut Vec<InstanceData>,
    young_deciduous: &mut Vec<InstanceData>,
    young_pine: &mut Vec<InstanceData>,
) {
    let x_off = config.min_distance + next_rng(rng) * (config.max_distance - config.min_distance);
    let z_jitter = (next_rng(rng) - 0.5) * config.spacing_m * 0.9;
    let kind = next_rng(rng);
    let age = next_rng(rng); // 0 = young, 1 = mature

    let dist_along = base_z + z_jitter as f64;
    let (px, pz, y, fx, fz) = interpolate_point(points, dist_along);
    let (wx, wz) = offset_position(px, pz, fx, fz, side * x_off);

    if kind < 0.35 {
        // Oak
        if age < 0.35 {
            let scale = 0.25 + next_rng(rng) * 0.25;
            young_deciduous.push(InstanceData { position: [wx, y, wz], scale });
        } else {
            let scale = config.min_scale + next_rng(rng) * (config.max_scale - config.min_scale);
            deciduous.push(InstanceData { position: [wx, y, wz], scale });
        }
    } else if kind < 0.65 {
        // Pine
        if age < 0.30 {
            let scale = 0.20 + next_rng(rng) * 0.25;
            young_pine.push(InstanceData { position: [wx, y, wz], scale });
        } else {
            let scale = config.min_scale + next_rng(rng) * (config.max_scale - config.min_scale);
            pine.push(InstanceData { position: [wx, y, wz], scale });
        }
    } else {
        // Bush / understory
        let scale = 0.5 + next_rng(rng) * 1.0;
        bush.push(InstanceData { position: [wx, y, wz], scale });
    }
}
