use std::f32::consts::TAU;

use crate::mesh::Vertex;

/// Add an ellipsoid centered at (cx, cy, cz) with radii (rx, ry, rz).
pub fn add_ellipsoid(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    cx: f32, cy: f32, cz: f32,
    rx: f32, ry: f32, rz: f32,
    stacks: usize, slices: usize,
) {
    let v_base = vertices.len() as u32;
    for i in 0..=stacks {
        let phi = std::f32::consts::PI * i as f32 / stacks as f32;
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();
        for j in 0..=slices {
            let theta = TAU * j as f32 / slices as f32;
            let nx = theta.cos() * sin_phi;
            let ny = cos_phi;
            let nz = theta.sin() * sin_phi;
            vertices.push(Vertex {
                position: [cx + nx * rx, cy + ny * ry, cz + nz * rz],
                normal: [nx, ny, nz],
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

/// Add a tapered cylinder between two points.
pub fn add_cylinder(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    base: [f32; 3], base_r: f32,
    top: [f32; 3], top_r: f32,
    sides: usize,
) {
    let ax = top[0] - base[0];
    let ay = top[1] - base[1];
    let az = top[2] - base[2];
    let len = (ax * ax + ay * ay + az * az).sqrt();
    if len < 0.0001 { return; }
    let (ax, ay, az) = (ax / len, ay / len, az / len);

    // Perpendicular frame
    let (ux, uy, uz) = if ax.abs() < 0.9 { (1.0, 0.0, 0.0) } else { (0.0, 1.0, 0.0) };
    let mut rx = ay * uz - az * uy;
    let mut ry = az * ux - ax * uz;
    let mut rz = ax * uy - ay * ux;
    let rlen = (rx * rx + ry * ry + rz * rz).sqrt();
    rx /= rlen; ry /= rlen; rz /= rlen;
    let fx = ry * az - rz * ay;
    let fy = rz * ax - rx * az;
    let fz = rx * ay - ry * ax;

    let v_base_idx = vertices.len() as u32;

    for section in 0..=1u32 {
        let t = section as f32;
        let r = base_r + (top_r - base_r) * t;
        let cx = base[0] + ax * len * t;
        let cy = base[1] + ay * len * t;
        let cz = base[2] + az * len * t;

        for i in 0..sides {
            let angle = TAU * i as f32 / sides as f32;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let nx = rx * cos_a + fx * sin_a;
            let ny = ry * cos_a + fy * sin_a;
            let nz = rz * cos_a + fz * sin_a;
            vertices.push(Vertex {
                position: [cx + nx * r, cy + ny * r, cz + nz * r],
                normal: [nx, ny, nz],
                uv: [i as f32 / sides as f32, t],
            });
        }
    }

    for i in 0..sides {
        let bl = v_base_idx + i as u32;
        let br = v_base_idx + ((i + 1) % sides) as u32;
        let tl = bl + sides as u32;
        let tr = br + sides as u32;
        indices.extend_from_slice(&[bl, tl, br, br, tl, tr]);
    }
}

/// Add a flat subdivided wing (one side).
pub fn add_wing(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    x_start: f32, x_end: f32,
    z_start: f32, z_end: f32,
    y: f32,
    cols: usize, rows: usize,
    flip: bool,
) {
    let v_base = vertices.len() as u32;
    for r in 0..=rows {
        for c in 0..=cols {
            let u = c as f32 / cols as f32;
            let v = r as f32 / rows as f32;
            let x = x_start + u * (x_end - x_start);
            let z = z_start + v * (z_end - z_start);
            let ny = if flip { -1.0 } else { 1.0 };
            vertices.push(Vertex {
                position: [x, y, z],
                normal: [0.0, ny, 0.0],
                uv: [u, v],
            });
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            let a = v_base + (r * (cols + 1) + c) as u32;
            let b = a + (cols + 1) as u32;
            if flip {
                indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            } else {
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }
}
