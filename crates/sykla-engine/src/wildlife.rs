use std::f32::consts::{PI, TAU};

use bytemuck::{Pod, Zeroable};

use crate::mesh::{CpuMesh, Vertex};
use crate::mesh_helpers::{add_cylinder, add_ellipsoid, add_wing};
use crate::terrain::{RoutePoint, DemTerrainSource, interpolate_point, interpolate_point_geo, offset_position};
use crate::vegetation::InstanceData;
use crate::water::WaterFeature;

// ── Animated instance data (32 bytes, vertex locations 3-6) ─────

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct AnimatedInstanceData {
    pub position: [f32; 3],
    pub scale: f32,
    pub velocity: [f32; 3],
    pub phase: f32,
}

impl AnimatedInstanceData {
    const ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        3 => Float32x3,
        4 => Float32,
        5 => Float32x3,
        6 => Float32,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<AnimatedInstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRS,
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Mesh generators
// ════════════════════════════════════════════════════════════════

/// Squirrel: 0.25m body, ellipsoid + bushy tail + tiny legs (~200 tris)
pub fn generate_squirrel() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Body ellipsoid: 0.24m long, 0.10m wide, 0.08m tall, Y center at 0.08
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.08, 0.0, 0.05, 0.04, 0.12, 5, 8);

    // Bushy tail curving upward
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.18, -0.14, 0.04, 0.06, 0.10, 4, 6);

    // Head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.10, 0.14, 0.03, 0.03, 0.03, 3, 6);

    // 4 tiny legs
    let legs: &[([f32; 3], [f32; 3])] = &[
        ([0.03, 0.0, 0.06], [0.03, 0.04, 0.06]),
        ([-0.03, 0.0, 0.06], [-0.03, 0.04, 0.06]),
        ([0.03, 0.0, -0.06], [0.03, 0.04, -0.06]),
        ([-0.03, 0.0, -0.06], [-0.03, 0.04, -0.06]),
    ];
    for (bottom, top) in legs {
        add_cylinder(&mut vertices, &mut indices, *bottom, 0.012, *top, 0.010, 4);
    }

    CpuMesh { vertices, indices }
}

/// Deer: 1.5m tall, cylinder body + neck + head + 4 tapered legs (~500 tris)
pub fn generate_deer() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Body barrel at Y=1.0
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 1.0, 0.0, 0.20, 0.22, 0.40, 6, 10);

    // 4 legs
    let legs: &[([f32; 3], [f32; 3], f32, f32)] = &[
        ([0.10, 0.0, 0.22], [0.10, 0.80, 0.22], 0.04, 0.035),
        ([-0.10, 0.0, 0.22], [-0.10, 0.80, 0.22], 0.04, 0.035),
        ([0.10, 0.0, -0.22], [0.10, 0.80, -0.22], 0.05, 0.035),
        ([-0.10, 0.0, -0.22], [-0.10, 0.80, -0.22], 0.05, 0.035),
    ];
    for (bottom, top, br, tr) in legs {
        add_cylinder(&mut vertices, &mut indices, *bottom, *br, *top, *tr, 6);
    }

    // Neck angled upward
    add_cylinder(
        &mut vertices, &mut indices,
        [0.0, 1.15, 0.35], 0.10,
        [0.0, 1.45, 0.48], 0.06,
        6,
    );

    // Head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 1.50, 0.52, 0.06, 0.06, 0.10, 4, 6);

    // Simple antlers (4 short sticks)
    add_cylinder(&mut vertices, &mut indices, [0.04, 1.52, 0.50], 0.012, [0.10, 1.72, 0.52], 0.005, 4);
    add_cylinder(&mut vertices, &mut indices, [0.08, 1.64, 0.51], 0.007, [0.14, 1.74, 0.54], 0.003, 4);
    add_cylinder(&mut vertices, &mut indices, [-0.04, 1.52, 0.50], 0.012, [-0.10, 1.72, 0.52], 0.005, 4);
    add_cylinder(&mut vertices, &mut indices, [-0.08, 1.64, 0.51], 0.007, [-0.14, 1.74, 0.54], 0.003, 4);

    CpuMesh { vertices, indices }
}

/// Turtle: 0.25m dome shell + flat underside + head + 4 paddles (~150 tris)
pub fn generate_turtle() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let shell_radius = 0.125;
    let shell_height = 0.08;
    let shell_y = 0.04;
    let stacks = 4;
    let slices = 8;

    // Shell dome (upper hemisphere)
    let v_base = vertices.len() as u32;
    for i in 0..=stacks {
        let phi = (PI * 0.5) * i as f32 / stacks as f32;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for j in 0..=slices {
            let theta = TAU * j as f32 / slices as f32;
            let x = theta.cos() * sin_phi * shell_radius;
            let y = shell_y + cos_phi * shell_height;
            let z = theta.sin() * sin_phi * shell_radius;
            let nx = theta.cos() * sin_phi;
            let ny = cos_phi * (shell_radius / shell_height);
            let nz = theta.sin() * sin_phi;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);
            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx / nlen, ny / nlen, nz / nlen],
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

    // Flat underside disc
    let disc_center = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, shell_y, 0.0],
        normal: [0.0, -1.0, 0.0],
        uv: [0.5, 0.5],
    });
    for j in 0..=slices {
        let theta = TAU * j as f32 / slices as f32;
        vertices.push(Vertex {
            position: [theta.cos() * shell_radius, shell_y, theta.sin() * shell_radius],
            normal: [0.0, -1.0, 0.0],
            uv: [0.5 + theta.cos() * 0.5, 0.5 + theta.sin() * 0.5],
        });
    }
    for j in 0..slices as u32 {
        indices.extend_from_slice(&[disc_center, disc_center + 1 + j + 1, disc_center + 1 + j]);
    }

    // Head stub
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.05, 0.14, 0.025, 0.02, 0.035, 3, 5);

    // 4 paddle legs
    let paddles: &[([f32; 3], [f32; 3])] = &[
        ([0.08, 0.02, 0.06], [0.12, 0.0, 0.08]),
        ([-0.08, 0.02, 0.06], [-0.12, 0.0, 0.08]),
        ([0.08, 0.02, -0.06], [0.12, 0.0, -0.08]),
        ([-0.08, 0.02, -0.06], [-0.12, 0.0, -0.08]),
    ];
    for (base, tip) in paddles {
        add_cylinder(&mut vertices, &mut indices, *base, 0.015, *tip, 0.008, 4);
    }

    CpuMesh { vertices, indices }
}

/// Egret: 0.9m tall standing bird with S-curved neck + beak + 2 long legs (~250 tris)
pub fn generate_egret() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Body
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.45, 0.0, 0.06, 0.08, 0.12, 5, 8);

    // S-curved neck (3 segments)
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.52, 0.08], 0.025, [0.0, 0.62, 0.05], 0.022, 5);
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.62, 0.05], 0.022, [0.0, 0.72, 0.08], 0.020, 5);
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.72, 0.08], 0.020, [0.0, 0.82, 0.10], 0.018, 5);

    // Head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.84, 0.11, 0.025, 0.022, 0.030, 3, 5);

    // Beak
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.84, 0.14], 0.008, [0.0, 0.83, 0.23], 0.002, 4);

    // 2 long thin legs
    add_cylinder(&mut vertices, &mut indices, [0.03, 0.0, 0.0], 0.012, [0.03, 0.38, 0.0], 0.010, 5);
    add_cylinder(&mut vertices, &mut indices, [-0.03, 0.0, 0.0], 0.012, [-0.03, 0.38, 0.0], 0.010, 5);

    CpuMesh { vertices, indices }
}

/// Parameterized flying bird with subdivided wings for shader-driven flapping.
/// Wings symmetric around X=0; `abs(position.x)` drives flap displacement.
/// (~250 tris)
pub fn generate_bird(body_radius: f32, wingspan_half: f32, chord: f32) -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let body_ry = body_radius * 0.7;
    let body_rz = body_radius * 1.5;

    // Body ellipsoid at origin
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.0, 0.0, body_radius, body_ry, body_rz, 5, 8);

    // Wings: 4x2 subdivided grid per side, both top and bottom faces
    let wing_cols = 4;
    let wing_rows = 2;
    let wing_y = 0.0;
    let x_inner = body_radius * 0.7;
    let z_half = chord * 0.5;

    // Right wing (top + bottom)
    add_wing(&mut vertices, &mut indices, x_inner, wingspan_half, -z_half, z_half, wing_y, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, x_inner, wingspan_half, -z_half, z_half, wing_y, wing_cols, wing_rows, true);

    // Left wing (top + bottom) — mirror X
    add_wing(&mut vertices, &mut indices, -wingspan_half, -x_inner, -z_half, z_half, wing_y, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, -wingspan_half, -x_inner, -z_half, z_half, wing_y, wing_cols, wing_rows, true);

    // Head
    let head_r = body_radius * 0.4;
    add_ellipsoid(&mut vertices, &mut indices, 0.0, body_ry * 0.3, body_rz * 1.2, head_r, head_r, head_r, 3, 5);

    // Tail fan
    let tb = vertices.len() as u32;
    let tail_len = body_rz * 0.7;
    vertices.push(Vertex { position: [0.0, 0.0, -body_rz], normal: [0.0, 0.3, -0.95], uv: [0.5, 0.0] });
    vertices.push(Vertex { position: [body_radius * 0.4, 0.0, -body_rz - tail_len], normal: [0.0, 0.3, -0.95], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [0.0, body_ry * 0.3, -body_rz - tail_len], normal: [0.0, 0.3, -0.95], uv: [0.5, 1.0] });
    vertices.push(Vertex { position: [-body_radius * 0.4, 0.0, -body_rz - tail_len], normal: [0.0, 0.3, -0.95], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[tb, tb + 1, tb + 2, tb, tb + 2, tb + 3]);

    CpuMesh { vertices, indices }
}

/// Small bird: 0.12m body, simple ellipsoid + 2 triangle wings (~100 tris)
pub fn generate_small_bird() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Body
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.0, 0.0, 0.035, 0.028, 0.06, 4, 6);

    // Head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.015, 0.065, 0.018, 0.018, 0.018, 3, 5);

    // Wings: 2x1 grid each side (still symmetric for shader flap)
    let wing_cols = 2;
    let wing_rows = 1;
    let x_inner = 0.025;
    let x_outer = 0.10;
    let z_half = 0.025;

    add_wing(&mut vertices, &mut indices, x_inner, x_outer, -z_half, z_half, 0.0, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, x_inner, x_outer, -z_half, z_half, 0.0, wing_cols, wing_rows, true);
    add_wing(&mut vertices, &mut indices, -x_outer, -x_inner, -z_half, z_half, 0.0, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, -x_outer, -x_inner, -z_half, z_half, 0.0, wing_cols, wing_rows, true);

    CpuMesh { vertices, indices }
}

/// Goose: 0.5m body, bird body + long curved neck + head + wings (~300 tris)
pub fn generate_goose() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let body_r = 0.08;
    let body_ry = 0.065;
    let body_rz = 0.15;

    // Body
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.0, 0.0, body_r, body_ry, body_rz, 5, 8);

    // Long curved neck (3 segments, extending forward and slightly up)
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.04, 0.14], 0.025, [0.0, 0.06, 0.22], 0.020, 5);
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.06, 0.22], 0.020, [0.0, 0.10, 0.28], 0.018, 5);
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.10, 0.28], 0.018, [0.0, 0.12, 0.33], 0.016, 5);

    // Head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.13, 0.35, 0.022, 0.020, 0.028, 3, 5);

    // Wings: 4x2 subdivided
    let wing_cols = 4;
    let wing_rows = 2;
    let x_inner = body_r * 0.7;
    let x_outer = 0.35;
    let z_half = 0.04;

    add_wing(&mut vertices, &mut indices, x_inner, x_outer, -z_half, z_half, 0.0, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, x_inner, x_outer, -z_half, z_half, 0.0, wing_cols, wing_rows, true);
    add_wing(&mut vertices, &mut indices, -x_outer, -x_inner, -z_half, z_half, 0.0, wing_cols, wing_rows, false);
    add_wing(&mut vertices, &mut indices, -x_outer, -x_inner, -z_half, z_half, 0.0, wing_cols, wing_rows, true);

    // Tail
    let tb = vertices.len() as u32;
    vertices.push(Vertex { position: [0.0, 0.0, -body_rz], normal: [0.0, 0.3, -0.95], uv: [0.5, 0.0] });
    vertices.push(Vertex { position: [0.03, 0.0, -body_rz - 0.06], normal: [0.0, 0.3, -0.95], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [0.0, 0.02, -body_rz - 0.06], normal: [0.0, 0.3, -0.95], uv: [0.5, 1.0] });
    vertices.push(Vertex { position: [-0.03, 0.0, -body_rz - 0.06], normal: [0.0, 0.3, -0.95], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[tb, tb + 1, tb + 2, tb, tb + 2, tb + 3]);

    CpuMesh { vertices, indices }
}

/// Turkey: plump ground bird with fan tail, short neck, wattle (~300 tris)
pub fn generate_turkey() -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Plump round body — exaggerated for cuteness
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.35, 0.0, 0.18, 0.20, 0.22, 6, 10);

    // Short thick neck angled up
    add_cylinder(
        &mut vertices, &mut indices,
        [0.0, 0.50, 0.18], 0.06,
        [0.0, 0.62, 0.24], 0.04,
        6,
    );

    // Small round head
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.66, 0.26, 0.035, 0.035, 0.04, 4, 6);

    // Red wattle (small ellipsoid hanging under beak)
    add_ellipsoid(&mut vertices, &mut indices, 0.0, 0.60, 0.30, 0.012, 0.025, 0.010, 3, 5);

    // Beak
    add_cylinder(&mut vertices, &mut indices, [0.0, 0.66, 0.30], 0.010, [0.0, 0.65, 0.36], 0.003, 4);

    // 2 thick legs
    add_cylinder(&mut vertices, &mut indices, [0.06, 0.0, 0.0], 0.025, [0.06, 0.20, 0.0], 0.020, 5);
    add_cylinder(&mut vertices, &mut indices, [-0.06, 0.0, 0.0], 0.025, [-0.06, 0.20, 0.0], 0.020, 5);

    // Fan tail — 7 flat triangles radiating from rear, angled up ~60°
    let tail_base = [0.0f32, 0.40, -0.20];
    let tail_len = 0.28;
    let fan_half = 7;
    for i in 0..fan_half {
        let t = i as f32 / (fan_half - 1) as f32; // 0..1
        let angle = (t - 0.5) * 1.2; // spread ~70 degrees total
        let up_angle: f32 = 1.05; // ~60 degrees up from horizontal
        let dx = angle.sin() * tail_len;
        let dy = up_angle.sin() * tail_len;
        let dz = -up_angle.cos() * tail_len * angle.cos();

        let tip = [tail_base[0] + dx, tail_base[1] + dy, tail_base[2] + dz];
        // Each feather is a thin triangle
        let side_offset = 0.015;
        let tb = vertices.len() as u32;
        let nx = -angle.sin();
        let nz = angle.cos();
        vertices.push(Vertex { position: tail_base, normal: [nx, 0.3, nz], uv: [0.5, 0.0] });
        vertices.push(Vertex { position: tip, normal: [nx, 0.3, nz], uv: [0.5, 1.0] });
        vertices.push(Vertex {
            position: [tail_base[0] + side_offset * (1.0 - t * 2.0), tail_base[1] + 0.02, tail_base[2] - 0.02],
            normal: [nx, 0.3, nz],
            uv: [1.0, 0.0],
        });
        indices.extend_from_slice(&[tb, tb + 1, tb + 2]);
    }

    CpuMesh { vertices, indices }
}

// ════════════════════════════════════════════════════════════════
// Placement
// ════════════════════════════════════════════════════════════════

pub struct WildlifePlacement {
    // Ground animals (static, use InstanceData)
    pub squirrels: Vec<InstanceData>,
    pub deer: Vec<InstanceData>,
    pub turkeys: Vec<InstanceData>,
    pub turtles: Vec<InstanceData>,
    pub egrets: Vec<InstanceData>,
    // Flying animals (animated, use AnimatedInstanceData)
    pub crows: Vec<AnimatedInstanceData>,
    pub hawks: Vec<AnimatedInstanceData>,
    pub starlings: Vec<AnimatedInstanceData>,
    pub geese: Vec<AnimatedInstanceData>,
}

pub fn place_wildlife(
    points: &[RoutePoint],
    water_features: &[WaterFeature],
    dem: Option<&DemTerrainSource>,
) -> WildlifePlacement {
    let total_dist = points.last().map(|p| p.distance_m as f32).unwrap_or(0.0);
    let mut rng: u64 = 98765;

    // Precompute water proximity: for each 10m slot, distance to nearest water
    let num_slots = (total_dist / 10.0) as usize;
    let water_proximity: Vec<f32> = (0..num_slots)
        .map(|s| {
            let d = s as f32 * 10.0;
            water_features
                .iter()
                .map(|w| (d - w.route_distance).abs())
                .fold(f32::MAX, f32::min)
        })
        .collect();

    let mut squirrels = Vec::new();
    let mut deer = Vec::new();
    let mut turkeys = Vec::new();
    let mut turtles = Vec::new();
    let mut egrets = Vec::new();
    let mut crows = Vec::new();
    let mut hawks = Vec::new();
    let mut starlings = Vec::new();
    let mut geese = Vec::new();

    // ── Ground animals ──

    // Squirrels: every 15m slot, 0-4 per slot
    {
        let slot_spacing = 15.0;
        let n = (total_dist / slot_spacing) as usize;
        for slot in 0..n {
            let base_dist = slot as f32 * slot_spacing;
            let prox_idx = (base_dist / 10.0) as usize;
            let near_water = prox_idx < water_proximity.len()
                && water_proximity[prox_idx] < 30.0;
            let max_count = if near_water { 4 } else { 3 };
            let count = (next_rng(&mut rng) * (max_count as f32 + 1.0)) as usize;

            for _ in 0..count {
                let side = if next_rng(&mut rng) > 0.5 { 1.0 } else { -1.0 };
                let lateral = side * (5.0 + next_rng(&mut rng) * 25.0);
                let z_jitter = (next_rng(&mut rng) - 0.5) * slot_spacing;
                let dist_along = (base_dist + z_jitter) as f64;
                let (px, pz, y, fx, fz, gx, gz) = interpolate_point_geo(points, dist_along);
                let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
                let y = if let Some(d) = dem { d.sample_at(gx, gz, fx, fz, lateral).unwrap_or(y) } else { y };
                let scale = 1.0 + next_rng(&mut rng) * 0.5;
                squirrels.push(InstanceData { position: [wx, y, wz], scale });
            }
        }
    }

    // Deer: every 100m, 45% chance of group (3-6)
    {
        let slot_spacing = 100.0;
        let n = (total_dist / slot_spacing) as usize;
        for slot in 0..n {
            let base_dist = slot as f32 * slot_spacing;
            if next_rng(&mut rng) > 0.45 { continue; }

            let prox_idx = (base_dist / 10.0) as usize;
            let near_water = prox_idx < water_proximity.len()
                && water_proximity[prox_idx] < 40.0;

            let group_size = 3 + (next_rng(&mut rng) * 4.0) as usize;
            let extra = if near_water { 1 + (next_rng(&mut rng) * 2.0) as usize } else { 0 };

            let side = if next_rng(&mut rng) > 0.5 { 1.0 } else { -1.0 };
            let base_lateral = side * (20.0 + next_rng(&mut rng) * 40.0);

            for i in 0..(group_size + extra) {
                let lateral = base_lateral + (next_rng(&mut rng) - 0.5) * 10.0;
                let dist_along = (base_dist + (next_rng(&mut rng) - 0.5) * 20.0) as f64;
                let (px, pz, y, fx, fz, gx, gz) = interpolate_point_geo(points, dist_along);
                let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
                let y = if let Some(d) = dem { d.sample_at(gx, gz, fx, fz, lateral).unwrap_or(y) } else { y };
                let scale = 0.9 + next_rng(&mut rng) * 0.40;
                let scale = if near_water && i >= group_size { scale * 0.9 } else { scale };
                deer.push(InstanceData { position: [wx, y, wz], scale });
            }
        }
    }

    // Turkeys: every 120m, 25% chance of group (2-4)
    {
        let slot_spacing = 120.0;
        let n = (total_dist / slot_spacing) as usize;
        for slot in 0..n {
            let base_dist = slot as f32 * slot_spacing;
            if next_rng(&mut rng) > 0.25 { continue; }

            let group_size = 2 + (next_rng(&mut rng) * 3.0) as usize;
            let side = if next_rng(&mut rng) > 0.5 { 1.0 } else { -1.0 };
            let base_lateral = side * (8.0 + next_rng(&mut rng) * 27.0);

            for _ in 0..group_size {
                let lateral = base_lateral + (next_rng(&mut rng) - 0.5) * 8.0;
                let dist_along = (base_dist + (next_rng(&mut rng) - 0.5) * 15.0) as f64;
                let (px, pz, y, fx, fz, gx, gz) = interpolate_point_geo(points, dist_along);
                let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
                let y = if let Some(d) = dem { d.sample_at(gx, gz, fx, fz, lateral).unwrap_or(y) } else { y };
                let scale = 0.9 + next_rng(&mut rng) * 0.3;
                turkeys.push(InstanceData { position: [wx, y, wz], scale });
            }
        }
    }

    // Turtles: only within 10m of water, 1-3 per feature
    for wf in water_features {
        let count = 1 + (next_rng(&mut rng) * 3.0) as usize;
        for _ in 0..count {
            let dist_along = wf.route_distance + (next_rng(&mut rng) - 0.5) * wf.z_extent;
            let lateral = wf.lateral_offset + (next_rng(&mut rng) - 0.5) * wf.x_extent * 0.5;
            let (px, pz, _elev, fx, fz) = interpolate_point(points, dist_along as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
            let y = wf.elevation + 0.2;
            turtles.push(InstanceData { position: [wx, y, wz], scale: 0.8 + next_rng(&mut rng) * 0.4 });
        }
    }

    // Egrets: only within 15m of water, 0-2 per feature, standing at edge
    for wf in water_features {
        let count = (next_rng(&mut rng) * 3.0) as usize;
        for _ in 0..count {
            let side = if next_rng(&mut rng) > 0.5 { 0.5 } else { -0.5 };
            let dist_along = wf.route_distance + (next_rng(&mut rng) - 0.5) * wf.z_extent * 0.8;
            let lateral = wf.lateral_offset + side * wf.x_extent;
            let (px, pz, _elev, fx, fz) = interpolate_point(points, dist_along as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
            let y = wf.elevation + 0.1;
            egrets.push(InstanceData { position: [wx, y, wz], scale: 0.9 + next_rng(&mut rng) * 0.2 });
        }
    }

    // ── Flying animals ──
    // Flying animals use world-space positions. We compute initial world positions
    // from route distance + lateral offset, then they fly freely in world space.

    // Crows: 1 per ~120m
    {
        let n = (total_dist / 120.0) as usize;
        for i in 0..n {
            let dist_along = i as f32 * 120.0 + next_rng(&mut rng) * 60.0;
            let lateral = (next_rng(&mut rng) - 0.5) * 60.0;
            let (px, pz, base_elev, fx, fz) = interpolate_point(points, dist_along as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
            let alt = 15.0 + next_rng(&mut rng) * 15.0;
            // Velocity in world space: forward along route direction + some lateral drift
            let speed_along = 3.0 + next_rng(&mut rng) * 1.0;
            let speed_lateral = (next_rng(&mut rng) - 0.5) * 1.0;
            crows.push(AnimatedInstanceData {
                position: [wx, base_elev + alt, wz],
                scale: 1.0,
                velocity: [fx * speed_along + (-fz) * speed_lateral, 0.0, fz * speed_along + fx * speed_lateral],
                phase: next_rng(&mut rng) * TAU,
            });
        }
    }

    // Hawks/vultures: 3-6 total, high altitude, slow
    {
        let count = 3 + (next_rng(&mut rng) * 4.0) as usize;
        for _ in 0..count {
            let dist_along = next_rng(&mut rng) * total_dist;
            let lateral = (next_rng(&mut rng) - 0.5) * 80.0;
            let (px, pz, base_elev, fx, fz) = interpolate_point(points, dist_along as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, lateral);
            let alt = 40.0 + next_rng(&mut rng) * 40.0;
            let speed_along = 1.5 + next_rng(&mut rng) * 1.0;
            let speed_lateral = (next_rng(&mut rng) - 0.5) * 0.5;
            hawks.push(AnimatedInstanceData {
                position: [wx, base_elev + alt, wz],
                scale: 1.0,
                velocity: [fx * speed_along + (-fz) * speed_lateral, 0.0, fz * speed_along + fx * speed_lateral],
                phase: next_rng(&mut rng) * TAU,
            });
        }
    }

    // Starling flocks: 3-7 flocks of 25-60 birds each
    {
        let num_flocks = 3 + (next_rng(&mut rng) * 5.0) as usize;
        for _ in 0..num_flocks {
            let flock_dist = next_rng(&mut rng) * total_dist;
            let flock_lateral = (next_rng(&mut rng) - 0.5) * 60.0;
            let (fpx, fpz, base_elev, ffx, ffz) = interpolate_point(points, flock_dist as f64);
            let (fwx, fwz) = offset_position(fpx, fpz, ffx, ffz, flock_lateral);
            let flock_alt = 10.0 + next_rng(&mut rng) * 15.0;
            let flock_speed = 4.0 + next_rng(&mut rng) * 2.0;
            let flock_drift = (next_rng(&mut rng) - 0.5) * 2.0;
            let flock_size = 25 + (next_rng(&mut rng) * 35.0) as usize;

            for _ in 0..flock_size {
                let scatter = 3.0;
                let sx = fwx + (next_rng(&mut rng) - 0.5) * scatter;
                let sy = base_elev + flock_alt + (next_rng(&mut rng) - 0.5) * scatter * 0.5;
                let sz = fwz + (next_rng(&mut rng) - 0.5) * scatter;
                starlings.push(AnimatedInstanceData {
                    position: [sx, sy, sz],
                    scale: 1.0,
                    velocity: [
                        ffx * flock_speed + (-ffz) * flock_drift + (next_rng(&mut rng) - 0.5) * 0.5,
                        (next_rng(&mut rng) - 0.5) * 0.3,
                        ffz * flock_speed + ffx * flock_drift + (next_rng(&mut rng) - 0.5) * 0.5,
                    ],
                    phase: next_rng(&mut rng) * TAU,
                });
            }
        }
    }

    // Geese: 2-4 V-formations of 7-15 birds
    {
        let num_formations = 2 + (next_rng(&mut rng) * 3.0) as usize;
        for _ in 0..num_formations {
            let leader_dist = next_rng(&mut rng) * total_dist;
            let leader_lateral = (next_rng(&mut rng) - 0.5) * 40.0;
            let (lpx, lpz, base_elev, lfx, lfz) = interpolate_point(points, leader_dist as f64);
            let (lwx, lwz) = offset_position(lpx, lpz, lfx, lfz, leader_lateral);
            let alt = 20.0 + next_rng(&mut rng) * 20.0;
            let vel_speed = 5.0 + next_rng(&mut rng) * 2.0;
            let formation_size = 7 + (next_rng(&mut rng) * 8.0) as usize;

            // Perpendicular direction for V-formation spread
            let perp_x = -lfz;
            let perp_z = lfx;

            for i in 0..formation_size {
                let (dx, dd) = if i == 0 {
                    (0.0, 0.0)
                } else {
                    let arm = ((i + 1) / 2) as f32;
                    let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                    (side * arm * 2.5, -arm * 3.0)
                };

                let wx = lwx + perp_x * dx + lfx * dd;
                let wz = lwz + perp_z * dx + lfz * dd;

                geese.push(AnimatedInstanceData {
                    position: [wx, base_elev + alt, wz],
                    scale: 1.0,
                    velocity: [lfx * vel_speed + (next_rng(&mut rng) - 0.5) * 0.2, 0.0, lfz * vel_speed],
                    phase: i as f32 * 0.4,
                });
            }
        }
    }

    WildlifePlacement {
        squirrels, deer, turkeys, turtles, egrets,
        crows, hawks, starlings, geese,
    }
}

fn next_rng(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as u32 as f32) / (u32::MAX as f32)
}
