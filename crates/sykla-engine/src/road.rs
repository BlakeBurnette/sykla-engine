use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::{RoutePoint, SurfaceType};

pub struct RoadConfig {
    pub half_width: f32,
    pub y_offset: f32,
    pub elevation_scale: f32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            half_width: 5.0,
            y_offset: 0.08,
            elevation_scale: 1.0,
        }
    }
}

pub fn generate_road(points: &[RoutePoint], config: &RoadConfig) -> CpuMesh {
    let mut vertices = Vec::with_capacity(points.len() * 2);

    for point in points {
        let y = point.elevation_m as f32 * config.elevation_scale + config.y_offset;
        let v = point.distance_m as f32;
        let perp_x = -point.forward_z;
        let perp_z = point.forward_x;

        let banking = point.banking;
        let cos_b = banking.cos();
        let sin_b = banking.sin();
        let hw_horizontal = config.half_width * cos_b;

        // Normal tilted by banking angle (rotated around forward axis)
        let nx = -perp_x * sin_b;
        let ny = cos_b;
        let nz = -perp_z * sin_b;

        // Left edge (lower when banking > 0)
        vertices.push(Vertex {
            position: [
                point.pos_x + perp_x * (-hw_horizontal),
                y - config.half_width * sin_b,
                point.pos_z + perp_z * (-hw_horizontal),
            ],
            normal: [nx, ny, nz],
            uv: [0.0, v],
        });
        // Right edge (higher when banking > 0)
        vertices.push(Vertex {
            position: [
                point.pos_x + perp_x * hw_horizontal,
                y + config.half_width * sin_b,
                point.pos_z + perp_z * hw_horizontal,
            ],
            normal: [nx, ny, nz],
            uv: [1.0, v],
        });
    }

    let mut indices = Vec::with_capacity((points.len() - 1) * 6);
    for i in 0..(points.len() - 1) {
        let bl = (i * 2) as u32;
        let br = bl + 1;
        let tl = bl + 2;
        let tr = bl + 3;

        indices.push(bl);
        indices.push(tl);
        indices.push(br);
        indices.push(br);
        indices.push(tl);
        indices.push(tr);
    }

    CpuMesh { vertices, indices }
}

/// Generate separate road meshes for each contiguous surface type segment.
pub fn generate_roads_by_surface(points: &[RoutePoint], config: &RoadConfig) -> Vec<(CpuMesh, SurfaceType)> {
    if points.len() < 2 {
        return vec![];
    }

    let mut result = Vec::new();
    let mut seg_start = 0;

    while seg_start < points.len() {
        let surface = points[seg_start].surface;
        let mut seg_end = seg_start + 1;
        while seg_end < points.len() && points[seg_end].surface == surface {
            seg_end += 1;
        }
        if seg_end - seg_start >= 2 {
            let mesh = generate_road(&points[seg_start..seg_end], config);
            result.push((mesh, surface));
        }
        seg_start = seg_end;
    }

    result
}
