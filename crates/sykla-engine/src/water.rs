use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::{RoutePoint, interpolate_point, offset_position};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    Stream,
    Pond,
}

#[derive(Debug, Clone)]
pub struct WaterFeature {
    /// Distance along route to water center.
    pub route_distance: f32,
    /// Along-route extent.
    pub z_extent: f32,
    /// Lateral offset from road center (positive = right).
    pub lateral_offset: f32,
    /// Lateral extent.
    pub x_extent: f32,
    pub elevation: f32,
    pub kind: WaterKind,
}

/// Detect valley minima in the route elevation profile and place water features.
/// Uses 5-point smoothed average; points lower than neighbors by >2m become water.
pub fn detect_water_features(points: &[RoutePoint]) -> Vec<WaterFeature> {
    if points.len() < 12 {
        return Vec::new();
    }

    let elevations: Vec<f32> = points.iter().map(|p| p.elevation_m as f32).collect();
    let n = elevations.len();

    // 5-point moving average
    let smoothed: Vec<f32> = (0..n)
        .map(|i| {
            let start = i.saturating_sub(2);
            let end = (i + 3).min(n);
            let sum: f32 = elevations[start..end].iter().sum();
            sum / (end - start) as f32
        })
        .collect();

    let mut features = Vec::new();
    let mut rng: u64 = 12345;
    let mut last_dist = f32::NEG_INFINITY;

    for i in 5..n.saturating_sub(5) {
        let here = smoothed[i];
        let left_max = smoothed[i.saturating_sub(5)..i]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let right_max = smoothed[i + 1..(i + 6).min(n)]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        let depth = (left_max - here).min(right_max - here);
        if depth < 2.0 {
            continue;
        }

        let dist = points[i].distance_m as f32;

        // Don't place features too close together
        if dist - last_dist < 80.0 {
            continue;
        }

        let (kind, z_ext, x_ext) = if depth > 5.0 {
            (
                WaterKind::Pond,
                15.0 + next_rng(&mut rng) * 15.0,
                8.0 + next_rng(&mut rng) * 7.0,
            )
        } else {
            (
                WaterKind::Stream,
                20.0 + next_rng(&mut rng) * 40.0,
                3.0 + next_rng(&mut rng) * 3.0,
            )
        };

        let side = if next_rng(&mut rng) > 0.5 { 1.0 } else { -1.0 };
        let lateral = side * (15.0 + next_rng(&mut rng) * 25.0);

        features.push(WaterFeature {
            route_distance: dist,
            z_extent: z_ext,
            lateral_offset: lateral,
            x_extent: x_ext,
            elevation: here - 0.3,
            kind,
        });
        last_dist = dist;
    }

    features
}

/// Generate a subdivided flat water mesh for vertex-shader wave displacement.
/// Uses route points to place in world space along the curved path.
pub fn generate_water_mesh(feature: &WaterFeature, points: &[RoutePoint]) -> CpuMesh {
    let (cols, rows) = match feature.kind {
        WaterKind::Stream => (8, 16),
        WaterKind::Pond => (8, 8),
    };

    let mut vertices = Vec::with_capacity((cols + 1) * (rows + 1));
    let mut indices = Vec::with_capacity(cols * rows * 6);

    let y = feature.elevation;

    for r in 0..=rows {
        for c in 0..=cols {
            let u = c as f32 / cols as f32;
            let v = r as f32 / rows as f32;

            // Route-relative coordinates
            let along = feature.route_distance + (v - 0.5) * feature.z_extent;
            let lateral = feature.lateral_offset + (u - 0.5) * feature.x_extent;

            // Convert to world space
            let (px, pz, _elev, fx, fz) = interpolate_point(points, along as f64);
            let (wx, wz) = offset_position(px, pz, fx, fz, lateral);

            vertices.push(Vertex {
                position: [wx, y, wz],
                normal: [0.0, 1.0, 0.0],
                uv: [u, v],
            });
        }
    }

    for r in 0..rows {
        for c in 0..cols {
            let tl = (r * (cols + 1) + c) as u32;
            let tr = tl + 1;
            let bl = tl + (cols + 1) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    CpuMesh { vertices, indices }
}

fn next_rng(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as u32 as f32) / (u32::MAX as f32)
}
