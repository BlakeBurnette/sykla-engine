use sykla_core::types::RoutePoint;

/// Terrain mesh data generated from route elevation profile.
/// Uses a cross-section extrusion approach: for each route point,
/// generate a terrain cross-section perpendicular to the route direction.
#[derive(Debug, Clone)]
pub struct TerrainMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Configuration for terrain generation.
#[derive(Debug, Clone)]
pub struct TerrainConfig {
    /// Width of terrain on each side of the road (meters).
    pub half_width: f32,
    /// Number of cross-section subdivisions on each side.
    pub cross_sections: usize,
    /// How quickly terrain drops off from road (higher = steeper sides).
    pub falloff: f32,
    /// Vertical exaggeration factor for elevation.
    pub elevation_scale: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            half_width: 50.0,
            cross_sections: 8,
            falloff: 0.3,
            elevation_scale: 1.0,
        }
    }
}

/// Generate a terrain mesh from route points.
/// The terrain is extruded along the route, with elevation from the GPX data.
/// The route path runs down the center (x=0), with terrain extending to each side.
///
/// Coordinate system:
/// - X: perpendicular to route (left/right)
/// - Y: up (elevation)
/// - Z: along route (distance from start)
pub fn generate_terrain(points: &[RoutePoint], config: &TerrainConfig) -> TerrainMesh {
    let n_points = points.len();
    if n_points < 2 {
        return TerrainMesh {
            vertices: vec![],
            normals: vec![],
            uvs: vec![],
            indices: vec![],
        };
    }

    let n_cross = config.cross_sections * 2 + 1; // left + center + right
    let n_verts = n_points * n_cross;
    let mut vertices = Vec::with_capacity(n_verts);
    let mut normals = Vec::with_capacity(n_verts);
    let mut uvs = Vec::with_capacity(n_verts);

    for (i, point) in points.iter().enumerate() {
        let z = point.distance_from_start_m as f32;
        let center_y = point.elevation_m as f32 * config.elevation_scale;

        for j in 0..n_cross {
            // x position: from -half_width to +half_width
            let t = j as f32 / (n_cross - 1) as f32; // 0 to 1
            let x = (t - 0.5) * 2.0 * config.half_width;

            // Elevation drops off from center
            let dist_from_center = x.abs() / config.half_width;
            let y_offset = -dist_from_center.powf(1.5) * config.half_width * config.falloff;
            let y = center_y + y_offset;

            vertices.push([x, y, z]);
            normals.push([0.0, 1.0, 0.0]); // approximate, will refine
            uvs.push([t, i as f32 / (n_points - 1) as f32]);
        }
    }

    // Generate triangle indices (triangle strip pattern)
    let mut indices = Vec::new();
    for i in 0..n_points - 1 {
        for j in 0..n_cross - 1 {
            let tl = (i * n_cross + j) as u32;
            let tr = tl + 1;
            let bl = ((i + 1) * n_cross + j) as u32;
            let br = bl + 1;

            // Two triangles per quad
            indices.push(tl);
            indices.push(bl);
            indices.push(tr);

            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    // Compute normals from triangle faces
    compute_normals(&mut normals, &vertices, &indices);

    TerrainMesh {
        vertices,
        normals,
        uvs,
        indices,
    }
}

fn compute_normals(normals: &mut Vec<[f32; 3]>, vertices: &[[f32; 3]], indices: &[u32]) {
    // Reset normals
    for n in normals.iter_mut() {
        *n = [0.0, 0.0, 0.0];
    }

    // Accumulate face normals
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let v0 = vertices[i0];
        let v1 = vertices[i1];
        let v2 = vertices[i2];

        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];

        for &idx in &[i0, i1, i2] {
            normals[idx][0] += nx;
            normals[idx][1] += ny;
            normals[idx][2] += nz;
        }
    }

    // Normalize
    for n in normals.iter_mut() {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0001 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
        } else {
            *n = [0.0, 1.0, 0.0];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sykla_core::types::RoutePoint;

    fn sample_points() -> Vec<RoutePoint> {
        vec![
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 100.0, distance_from_start_m: 0.0, grade_percent: Some(0.0) },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 105.0, distance_from_start_m: 100.0, grade_percent: Some(5.0) },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 110.0, distance_from_start_m: 200.0, grade_percent: Some(5.0) },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 108.0, distance_from_start_m: 300.0, grade_percent: Some(-2.0) },
        ]
    }

    #[test]
    fn test_generate_terrain_has_geometry() {
        let mesh = generate_terrain(&sample_points(), &TerrainConfig::default());
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.vertices.len(), mesh.normals.len());
        assert_eq!(mesh.vertices.len(), mesh.uvs.len());
    }

    #[test]
    fn test_terrain_vertex_count() {
        let config = TerrainConfig {
            cross_sections: 4,
            ..Default::default()
        };
        let points = sample_points();
        let mesh = generate_terrain(&points, &config);
        let expected_verts = points.len() * (config.cross_sections * 2 + 1);
        assert_eq!(mesh.vertices.len(), expected_verts);
    }

    #[test]
    fn test_empty_points() {
        let mesh = generate_terrain(&[], &TerrainConfig::default());
        assert!(mesh.vertices.is_empty());
    }
}
