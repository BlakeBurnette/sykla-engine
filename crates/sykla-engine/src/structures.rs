use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::RoutePoint;
use crate::vegetation::InstanceData;

/// Generate a cabin body mesh: box ~4x3x5m + triangular prism roof.
pub fn generate_cabin_body() -> CpuMesh {
    let hw = 2.0_f32; // half width (X)
    let hh = 1.5_f32; // half height (Y) — wall is 3m
    let hd = 2.5_f32; // half depth (Z)
    let roof_h = 1.5_f32; // roof ridge above wall top

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Box walls — 4 faces (no floor)
    // Front face (Z+)
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [-hw, 0.0, hd], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [ hw, 0.0, hd], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [ hw, hh * 2.0, hd], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [-hw, hh * 2.0, hd], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    // Back face (Z-)
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [ hw, 0.0, -hd], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [-hw, 0.0, -hd], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [-hw, hh * 2.0, -hd], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [ hw, hh * 2.0, -hd], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    // Left face (X-)
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [-hw, 0.0, -hd], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [-hw, 0.0,  hd], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [-hw, hh * 2.0,  hd], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [-hw, hh * 2.0, -hd], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    // Right face (X+)
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [hw, 0.0,  hd], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [hw, 0.0, -hd], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [hw, hh * 2.0, -hd], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [hw, hh * 2.0,  hd], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    // Roof — triangular prism
    let wall_top = hh * 2.0;
    let ridge_y = wall_top + roof_h;

    // Front gable triangle
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [-hw, wall_top, hd], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [ hw, wall_top, hd], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [0.0, ridge_y,  hd], normal: [0.0, 0.0, 1.0], uv: [0.5, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2]);

    // Back gable triangle
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [ hw, wall_top, -hd], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [-hw, wall_top, -hd], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [0.0, ridge_y,  -hd], normal: [0.0, 0.0, -1.0], uv: [0.5, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2]);

    // Left roof slope
    let n_left = [-roof_h, hw, 0.0_f32];
    let len = (n_left[0] * n_left[0] + n_left[1] * n_left[1]).sqrt();
    let n_left = [-roof_h / len, hw / len, 0.0];
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [-hw, wall_top, -hd], normal: n_left, uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [-hw, wall_top,  hd], normal: n_left, uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [0.0, ridge_y,   hd], normal: n_left, uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [0.0, ridge_y,  -hd], normal: n_left, uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    // Right roof slope
    let n_right = [roof_h / len, hw / len, 0.0];
    let vi = vertices.len() as u32;
    vertices.push(Vertex { position: [hw, wall_top,  hd], normal: n_right, uv: [0.0, 0.0] });
    vertices.push(Vertex { position: [hw, wall_top, -hd], normal: n_right, uv: [1.0, 0.0] });
    vertices.push(Vertex { position: [0.0, ridge_y, -hd], normal: n_right, uv: [1.0, 1.0] });
    vertices.push(Vertex { position: [0.0, ridge_y,  hd], normal: n_right, uv: [0.0, 1.0] });
    indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);

    CpuMesh { vertices, indices }
}

/// Generate cabin window quads — two small windows on the front face.
pub fn generate_cabin_windows() -> CpuMesh {
    let wall_top = 3.0_f32;
    let hd = 2.5_f32;
    let offset = 0.01; // Slightly in front of wall

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Two windows on front face (Z+), each 1.0m wide x 0.8m tall
    for &win_x in &[-0.8_f32, 0.8] {
        let vi = vertices.len() as u32;
        let y_bot = wall_top * 0.45;
        let y_top = y_bot + 0.8;
        let x_left = win_x - 0.5;
        let x_right = win_x + 0.5;
        let z = hd + offset;

        vertices.push(Vertex { position: [x_left,  y_bot, z], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] });
        vertices.push(Vertex { position: [x_right, y_bot, z], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] });
        vertices.push(Vertex { position: [x_right, y_top, z], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] });
        vertices.push(Vertex { position: [x_left,  y_top, z], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] });
        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
    }

    CpuMesh { vertices, indices }
}

/// Place cabins along the route, offset from road, only below treeline.
pub fn place_cabins(
    points: &[RoutePoint],
    treeline: f32,
    spacing_m: f32,
) -> Vec<InstanceData> {
    if spacing_m <= 0.0 || points.len() < 2 {
        return Vec::new();
    }

    let mut instances = Vec::new();
    let mut next_dist = spacing_m;

    for p in points {
        let d = p.distance_m as f32;
        if d < next_dist {
            continue;
        }
        next_dist = d + spacing_m;

        let elev = p.elevation_m as f32;
        if elev > treeline {
            continue;
        }

        let perp_x = -p.forward_z;
        let perp_z = p.forward_x;

        // Place on both sides of road
        for &side in &[-1.0_f32, 1.0] {
            let h = pos_hash(d as u32 + if side > 0.0 { 1000 } else { 0 }, 42);
            let offset = 25.0 + h * 35.0; // 25-60m from road
            let x = p.pos_x + perp_x * offset * side;
            let z = p.pos_z + perp_z * offset * side;

            instances.push(InstanceData {
                position: [x, elev, z],
                scale: 1.0,
            });
        }
    }

    instances
}

/// Generate a snow bank mesh: low elongated mound.
pub fn generate_snow_bank() -> CpuMesh {
    // Squashed ellipsoid: ~0.5m tall, 1.5m wide, 3m long
    let segments = 8;
    let rings = 4;
    let rx = 0.75_f32; // half-width
    let ry = 0.5_f32;  // height
    let rz = 1.5_f32;  // half-length

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Generate upper hemisphere of ellipsoid (only top half visible)
    for ring in 0..=rings {
        let phi = (ring as f32 / rings as f32) * std::f32::consts::FRAC_PI_2; // 0 to PI/2
        let y = ry * phi.sin();
        let ring_scale = phi.cos();

        for seg in 0..=segments {
            let theta = (seg as f32 / segments as f32) * std::f32::consts::TAU;
            let x = rx * ring_scale * theta.cos();
            let z = rz * ring_scale * theta.sin();

            // Normal of ellipsoid
            let nx = x / (rx * rx);
            let ny = y / (ry * ry);
            let nz = z / (rz * rz);
            let len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx / len, ny / len, nz / len],
                uv: [0.0, 0.0],
            });
        }
    }

    // Indices
    let row_len = (segments + 1) as u32;
    for ring in 0..rings as u32 {
        for seg in 0..segments as u32 {
            let i0 = ring * row_len + seg;
            let i1 = i0 + 1;
            let i2 = i0 + row_len;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    CpuMesh { vertices, indices }
}

/// Place snow banks along both road edges at a fixed interval.
pub fn place_snow_banks(
    points: &[RoutePoint],
    road_half_width: f32,
    snow_start_elev: f32,
) -> Vec<InstanceData> {
    let spacing = 4.0_f32;
    let mut instances = Vec::new();
    let mut next_dist = 0.0_f32;

    for p in points {
        let d = p.distance_m as f32;
        if d < next_dist {
            continue;
        }
        next_dist = d + spacing;

        let elev = p.elevation_m as f32;
        if elev < snow_start_elev {
            continue;
        }

        let perp_x = -p.forward_z;
        let perp_z = p.forward_x;
        let offset = road_half_width + 1.0;

        for &side in &[-1.0_f32, 1.0] {
            let x = p.pos_x + perp_x * offset * side;
            let z = p.pos_z + perp_z * offset * side;

            instances.push(InstanceData {
                position: [x, elev, z],
                scale: 1.0,
            });
        }
    }

    instances
}

fn pos_hash(a: u32, b: u32) -> f32 {
    let h = a
        .wrapping_mul(2654435761)
        .wrapping_add(b.wrapping_mul(2246822519));
    (h % 10000) as f32 / 10000.0
}
