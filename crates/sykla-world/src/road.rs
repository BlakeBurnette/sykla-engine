use sykla_core::types::RoutePoint;

/// A road ribbon mesh that follows the route path.
#[derive(Debug, Clone)]
pub struct RoadMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Configuration for road generation.
#[derive(Debug, Clone)]
pub struct RoadConfig {
    /// Half-width of the road in meters.
    pub half_width: f32,
    /// Small offset above terrain to prevent z-fighting.
    pub y_offset: f32,
    /// Elevation scale (should match terrain).
    pub elevation_scale: f32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            half_width: 3.0,
            y_offset: 0.05,
            elevation_scale: 1.0,
        }
    }
}

/// Generate a road ribbon mesh following the route.
/// The road is a flat strip slightly above the terrain surface.
pub fn generate_road(points: &[RoutePoint], config: &RoadConfig) -> RoadMesh {
    let n = points.len();
    if n < 2 {
        return RoadMesh {
            vertices: vec![],
            normals: vec![],
            uvs: vec![],
            indices: vec![],
        };
    }

    let mut vertices = Vec::with_capacity(n * 2);
    let mut normals = Vec::with_capacity(n * 2);
    let mut uvs = Vec::with_capacity(n * 2);

    let total_dist = points.last().unwrap().distance_from_start_m as f32;

    for point in points.iter() {
        let z = point.distance_from_start_m as f32;
        let y = point.elevation_m as f32 * config.elevation_scale + config.y_offset;

        // Left edge
        vertices.push([-config.half_width, y, z]);
        normals.push([0.0, 1.0, 0.0]);
        let v = if total_dist > 0.0 { z / total_dist } else { 0.0 };
        uvs.push([0.0, v]);

        // Right edge
        vertices.push([config.half_width, y, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([1.0, v]);
    }

    // Triangle indices
    let mut indices = Vec::with_capacity((n - 1) * 6);
    for i in 0..n - 1 {
        let base = (i * 2) as u32;
        // Two triangles per quad segment
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 1);

        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    RoadMesh {
        vertices,
        normals,
        uvs,
        indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sykla_core::types::RoutePoint;

    #[test]
    fn test_generate_road() {
        let points = vec![
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 100.0, distance_from_start_m: 0.0, grade_percent: Some(0.0), surface: Default::default() },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 105.0, distance_from_start_m: 100.0, grade_percent: Some(5.0), surface: Default::default() },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 110.0, distance_from_start_m: 200.0, grade_percent: Some(5.0), surface: Default::default() },
        ];

        let mesh = generate_road(&points, &RoadConfig::default());
        assert_eq!(mesh.vertices.len(), 6); // 3 points * 2 edges
        assert_eq!(mesh.indices.len(), 12); // 2 quads * 6 indices
    }
}
