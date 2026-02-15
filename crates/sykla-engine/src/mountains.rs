use glam::Vec3;

use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::RoutePoint;

/// Generate a ring of mountain peaks around the route for distant scenery.
/// Inner ring: 14 jagged ridged peaks ~3km from route center.
/// Outer ring: 10 simpler peaks ~4.5km out (faded by fog = depth layering).
/// `min_inner_radius`: minimum inner ring distance (use 4000+ for DEM routes
/// where terrain already extends 1.5km).
pub fn generate_mountain_ring_with_radius(points: &[RoutePoint], min_inner_radius: f32) -> CpuMesh {
    if points.len() < 2 {
        return CpuMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
    }

    // Compute route AABB center + elevation range
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    let mut min_elev = f64::MAX;
    let mut max_elev = f64::MIN;

    for p in points {
        min_x = min_x.min(p.pos_x);
        max_x = max_x.max(p.pos_x);
        min_z = min_z.min(p.pos_z);
        max_z = max_z.max(p.pos_z);
        min_elev = min_elev.min(p.elevation_m);
        max_elev = max_elev.max(p.elevation_m);
    }

    let center_x = (min_x + max_x) * 0.5;
    let center_z = (min_z + max_z) * 0.5;
    // Base well below lowest point so mountains are never floating above camera
    let base_elev = min_elev as f32 - 300.0;

    // Ring radius: far enough that peaks look like distant mountains
    let extent_x = (max_x - min_x) * 0.5;
    let extent_z = (max_z - min_z) * 0.5;
    let inner_ring_radius = min_inner_radius.max(extent_x.max(extent_z) + 2000.0);
    let outer_ring_radius = inner_ring_radius * 1.5;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Peak heights relative to route's max elevation — summits tower above the road
    let elev_range = (max_elev - min_elev) as f32;
    let inner_min_height = max_elev as f32 + 200.0 + elev_range * 0.3;
    let inner_height_var = 400.0 + elev_range * 0.5;

    // === Inner ring: 14 jagged ridged peaks ===
    generate_ridged_peaks(
        &mut vertices,
        &mut indices,
        center_x,
        center_z,
        base_elev,
        inner_ring_radius,
        14,  // num_peaks
        10,  // height_rings
        24,  // angular segments
        inner_min_height - base_elev,  // min extra height (above base)
        inner_height_var,              // height variation
        400.0,  // min base radius
        300.0,  // base radius variation
        true,   // full detail (ridges, asymmetry, fractal noise)
    );

    // === Outer ring: 10 simpler peaks (faded by fog for depth) ===
    let outer_min_height = max_elev as f32 + 100.0 + elev_range * 0.2;
    let outer_height_var = 300.0 + elev_range * 0.3;
    generate_ridged_peaks(
        &mut vertices,
        &mut indices,
        center_x,
        center_z,
        base_elev,
        outer_ring_radius,
        10,  // num_peaks
        5,   // height_rings (simpler)
        16,  // angular segments (simpler)
        outer_min_height - base_elev,  // min extra height
        outer_height_var,
        500.0,  // wider base
        350.0,
        false,  // simplified (fewer ridges, less noise)
    );

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

/// Convenience wrapper using the default 3km minimum inner ring radius.
pub fn generate_mountain_ring(points: &[RoutePoint]) -> CpuMesh {
    generate_mountain_ring_with_radius(points, 3000.0)
}

#[allow(clippy::too_many_arguments)]
fn generate_ridged_peaks(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    center_x: f32,
    center_z: f32,
    base_elev: f32,
    ring_radius: f32,
    num_peaks: usize,
    height_rings: usize,
    sides: usize,
    min_extra_height: f32,
    height_variation: f32,
    min_base_radius: f32,
    base_radius_variation: f32,
    full_detail: bool,
) {
    for peak_i in 0..num_peaks {
        let base_angle = (peak_i as f32 / num_peaks as f32) * std::f32::consts::TAU;
        let jitter = simple_hash(peak_i as u32, 0) * 0.15 - 0.075;
        let angle = base_angle + jitter;

        // Asymmetric peak: apex offset 10-20% from base center
        let lean_amount = if full_detail {
            0.1 + simple_hash(peak_i as u32, 10) * 0.1
        } else {
            0.05
        };
        let lean_angle = simple_hash(peak_i as u32, 11) * std::f32::consts::TAU;

        let peak_base_x = center_x + ring_radius * angle.cos();
        let peak_base_z = center_z + ring_radius * angle.sin();

        let peak_height = base_elev + min_extra_height + simple_hash(peak_i as u32, 1) * height_variation;
        let base_radius = min_base_radius + simple_hash(peak_i as u32, 2) * base_radius_variation;

        // Varied shape: tall/narrow vs wide/shorter via hash
        let shape_t = simple_hash(peak_i as u32, 12);
        let width_scale = 0.7 + shape_t * 0.6; // 0.7..1.3
        let actual_radius = base_radius * width_scale;

        // Generate 3-5 ridge azimuths (minimum 45deg apart)
        let num_ridges = if full_detail {
            3 + (simple_hash(peak_i as u32, 3) * 3.0) as usize // 3-5
        } else {
            2 + (simple_hash(peak_i as u32, 3) * 2.0) as usize // 2-3
        };
        let mut ridge_angles = Vec::with_capacity(num_ridges);
        let first_ridge = simple_hash(peak_i as u32, 4) * std::f32::consts::TAU;
        ridge_angles.push(first_ridge);
        for r in 1..num_ridges {
            // Spread ridges with minimum 45deg separation
            let candidate = first_ridge + (r as f32 / num_ridges as f32) * std::f32::consts::TAU
                + (simple_hash(peak_i as u32 * 10 + r as u32, 5) - 0.5) * 0.4;
            ridge_angles.push(candidate);
        }

        let vert_offset = vertices.len() as u32;

        // Generate vertices: concentric rings from base to tip
        for ring in 0..=height_rings {
            let t = ring as f32 / height_rings as f32;
            // Base ring radius narrows sharply toward peak
            let ring_base_r = actual_radius * (1.0 - t * 0.95);
            let ring_y = base_elev + (peak_height - base_elev) * t;

            // Apex lean offset grows with height
            let lean_offset_x = lean_amount * actual_radius * lean_angle.cos() * t;
            let lean_offset_z = lean_amount * actual_radius * lean_angle.sin() * t;

            for seg in 0..sides {
                let seg_angle = (seg as f32 / sides as f32) * std::f32::consts::TAU;

                // Ridge-based radius modulation:
                // Vertices near ridge azimuths keep full radius.
                // Vertices between ridges are moderately concave.
                let ridge_prox = ridge_proximity(&ridge_angles, seg_angle);
                // ridge_prox = 1.0 on ridge, 0.0 far from any ridge

                // Concavity is moderate near base, minimal near apex
                let concave_strength = 0.15 + 0.20 * (1.0 - t); // 0.15..0.35
                let ridge_factor = 1.0 - concave_strength * (1.0 - ridge_prox);
                let r = ring_base_r * ridge_factor;

                // Fractal noise for natural roughness
                let noise = if full_detail {
                    // 2 octaves: +-20% large, +-8% fine
                    let n1 = simple_hash(peak_i as u32 * 100 + ring as u32 * 30 + seg as u32, 6)
                        * 0.40 - 0.20;
                    let n2 = simple_hash(peak_i as u32 * 200 + ring as u32 * 50 + seg as u32, 7)
                        * 0.16 - 0.08;
                    n1 + n2
                } else {
                    // Simpler: one octave +-15%
                    simple_hash(peak_i as u32 * 100 + ring as u32 * 20 + seg as u32, 3)
                        * 0.30 - 0.15
                };
                let r = r * (1.0 + noise);

                let x = peak_base_x + lean_offset_x + r * seg_angle.cos();
                let z = peak_base_z + lean_offset_z + r * seg_angle.sin();

                vertices.push(Vertex {
                    position: [x, ring_y, z],
                    normal: [0.0, 1.0, 0.0], // Recomputed after
                    uv: [0.0, 0.0],
                });
            }
        }

        // Apex vertex (offset by full lean)
        let apex_idx = vertices.len() as u32;
        let apex_lean_x = lean_amount * actual_radius * lean_angle.cos();
        let apex_lean_z = lean_amount * actual_radius * lean_angle.sin();
        vertices.push(Vertex {
            position: [
                peak_base_x + apex_lean_x,
                peak_height,
                peak_base_z + apex_lean_z,
            ],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });

        // Generate indices for mountain body
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
}

/// Compute proximity to nearest ridge azimuth (0.0 = far, 1.0 = on ridge).
/// Uses a smooth falloff so ridges blend naturally into valleys.
fn ridge_proximity(ridge_angles: &[f32], seg_angle: f32) -> f32 {
    let mut max_prox = 0.0_f32;
    for &ra in ridge_angles {
        let mut delta = (seg_angle - ra).abs();
        if delta > std::f32::consts::PI {
            delta = std::f32::consts::TAU - delta;
        }
        // Ridge influence: wide smooth falloff over ~60 degrees
        let ridge_width = 1.0; // radians (~57deg)
        let prox = (1.0 - delta / ridge_width).max(0.0);
        // Smoothstep for gradual transition (not squared — too sharp)
        let prox = prox * prox * (3.0 - 2.0 * prox);
        max_prox = max_prox.max(prox);
    }
    max_prox
}

/// Simple deterministic hash returning 0.0..1.0
fn simple_hash(a: u32, b: u32) -> f32 {
    let h = a
        .wrapping_mul(2654435761)
        .wrapping_add(b.wrapping_mul(2246822519));
    (h % 10000) as f32 / 10000.0
}
