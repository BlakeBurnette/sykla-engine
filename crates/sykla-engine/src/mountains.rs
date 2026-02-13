use glam::Vec3;

use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::RoutePoint;

/// Generate a ring of mountain peaks around the route for distant scenery.
pub fn generate_mountain_ring(points: &[RoutePoint]) -> CpuMesh {
    if points.len() < 2 {
        return CpuMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
    }

    // Compute route AABB center + average elevation
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    let mut sum_elev = 0.0_f64;

    for p in points {
        min_x = min_x.min(p.pos_x);
        max_x = max_x.max(p.pos_x);
        min_z = min_z.min(p.pos_z);
        max_z = max_z.max(p.pos_z);
        sum_elev += p.elevation_m;
    }

    let center_x = (min_x + max_x) * 0.5;
    let center_z = (min_z + max_z) * 0.5;
    let avg_elev = (sum_elev / points.len() as f64) as f32;
    let base_elev = avg_elev - 50.0;

    // Ring radius: at least 800m from center
    let extent_x = (max_x - min_x) * 0.5;
    let extent_z = (max_z - min_z) * 0.5;
    let ring_radius = 800.0_f32.max(extent_x.max(extent_z) + 500.0);

    let num_peaks = 12;
    let sides = 12;
    let height_rings = 4;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for peak_i in 0..num_peaks {
        let base_angle =
            (peak_i as f32 / num_peaks as f32) * std::f32::consts::TAU;
        // Angular jitter using simple hash
        let jitter = simple_hash(peak_i as u32, 0) * 0.15 - 0.075;
        let angle = base_angle + jitter;

        let peak_x = center_x + ring_radius * angle.cos();
        let peak_z = center_z + ring_radius * angle.sin();

        // Randomized peak parameters
        let peak_height = base_elev + 200.0 + simple_hash(peak_i as u32, 1) * 400.0;
        let base_radius = 250.0 + simple_hash(peak_i as u32, 2) * 150.0;

        let vert_offset = vertices.len() as u32;

        // Generate cone vertices: concentric rings from base to tip
        for ring in 0..=height_rings {
            let t = ring as f32 / height_rings as f32;
            let ring_radius_here = base_radius * (1.0 - t * 0.85); // Narrows toward peak
            let ring_y = base_elev + (peak_height - base_elev) * t;

            for seg in 0..sides {
                let seg_angle = (seg as f32 / sides as f32) * std::f32::consts::TAU;

                // Noise displacement for natural shape
                let noise = simple_hash(peak_i as u32 * 100 + ring * 20 + seg as u32, 3)
                    * 0.3
                    - 0.15;
                let r = ring_radius_here * (1.0 + noise);

                let x = peak_x + r * seg_angle.cos();
                let z = peak_z + r * seg_angle.sin();

                vertices.push(Vertex {
                    position: [x, ring_y, z],
                    normal: [0.0, 1.0, 0.0], // Recomputed below
                    uv: [0.0, 0.0],
                });
            }
        }

        // Apex vertex
        let apex_idx = vertices.len() as u32;
        vertices.push(Vertex {
            position: [peak_x, peak_height, peak_z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });

        // Generate indices for cone body
        for ring in 0..height_rings {
            for seg in 0..sides {
                let next_seg = (seg + 1) % sides;
                let curr_row = vert_offset + ring as u32 * sides as u32;
                let next_row = vert_offset + (ring as u32 + 1) * sides as u32;

                let i0 = curr_row + seg as u32;
                let i1 = curr_row + next_seg as u32;
                let i2 = next_row + seg as u32;
                let i3 = next_row + next_seg as u32;

                indices.push(i0);
                indices.push(i2);
                indices.push(i1);
                indices.push(i1);
                indices.push(i2);
                indices.push(i3);
            }
        }

        // Top cap: connect last ring to apex
        let last_ring = vert_offset + height_rings as u32 * sides as u32;
        for seg in 0..sides {
            let next_seg = (seg + 1) % sides;
            indices.push(last_ring + seg as u32);
            indices.push(apex_idx);
            indices.push(last_ring + next_seg as u32);
        }
    }

    // Recompute normals (same pattern as generate_terrain)
    let mut normals = vec![Vec3::ZERO; vertices.len()];
    for tri in indices.chunks(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        let v0 = Vec3::from(vertices[i0].position);
        let v1 = Vec3::from(vertices[i1].position);
        let v2 = Vec3::from(vertices[i2].position);

        let n = (v1 - v0).cross(v2 - v0);
        normals[i0] += n;
        normals[i1] += n;
        normals[i2] += n;
    }

    for (i, n) in normals.iter().enumerate() {
        let n = n.normalize_or_zero();
        vertices[i].normal = [n.x, n.y, n.z];
    }

    CpuMesh { vertices, indices }
}

/// Simple deterministic hash returning 0.0..1.0
fn simple_hash(a: u32, b: u32) -> f32 {
    let h = a
        .wrapping_mul(2654435761)
        .wrapping_add(b.wrapping_mul(2246822519));
    (h % 10000) as f32 / 10000.0
}
