use crate::mesh::{CpuMesh, Vertex};
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceType {
    Paved,
    Gravel,
}

impl Default for SurfaceType {
    fn default() -> Self {
        Self::Paved
    }
}

/// Route point with distance, elevation, 2D path position/direction, and surface type.
pub struct RoutePoint {
    pub distance_m: f64,
    pub elevation_m: f64,
    /// World X position of road center.
    pub pos_x: f32,
    /// World Z position of road center.
    pub pos_z: f32,
    /// Unit forward direction X.
    pub forward_x: f32,
    /// Unit forward direction Z.
    pub forward_z: f32,
    /// Surface type at this point.
    pub surface: SurfaceType,
}

impl Default for RoutePoint {
    fn default() -> Self {
        Self {
            distance_m: 0.0,
            elevation_m: 0.0,
            pos_x: 0.0,
            pos_z: 0.0,
            forward_x: 0.0,
            forward_z: 1.0,
            surface: SurfaceType::Paved,
        }
    }
}

/// Look up the surface type at a given distance along the route.
pub fn surface_at(points: &[RoutePoint], distance: f64) -> SurfaceType {
    if points.is_empty() {
        return SurfaceType::Paved;
    }
    for i in 1..points.len() {
        if points[i].distance_m >= distance {
            return points[i - 1].surface;
        }
    }
    points.last().unwrap().surface
}

/// Interpolate position, elevation, and forward direction at an arbitrary distance along the route.
/// Returns (pos_x, pos_z, elevation, forward_x, forward_z).
pub fn interpolate_point(points: &[RoutePoint], dist: f64) -> (f32, f32, f32, f32, f32) {
    if points.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 1.0);
    }
    if dist <= points[0].distance_m {
        let p = &points[0];
        return (p.pos_x, p.pos_z, p.elevation_m as f32, p.forward_x, p.forward_z);
    }
    for i in 1..points.len() {
        if points[i].distance_m >= dist {
            let prev = &points[i - 1];
            let curr = &points[i];
            let seg_len = curr.distance_m - prev.distance_m;
            let t = if seg_len.abs() < 0.001 {
                0.0
            } else {
                ((dist - prev.distance_m) / seg_len).clamp(0.0, 1.0) as f32
            };
            let px = prev.pos_x + t * (curr.pos_x - prev.pos_x);
            let pz = prev.pos_z + t * (curr.pos_z - prev.pos_z);
            let elev = prev.elevation_m as f32 + t * (curr.elevation_m - prev.elevation_m) as f32;
            // Interpolate forward direction and re-normalize
            let fx = prev.forward_x + t * (curr.forward_x - prev.forward_x);
            let fz = prev.forward_z + t * (curr.forward_z - prev.forward_z);
            let len = (fx * fx + fz * fz).sqrt().max(0.0001);
            return (px, pz, elev, fx / len, fz / len);
        }
    }
    let p = points.last().unwrap();
    (p.pos_x, p.pos_z, p.elevation_m as f32, p.forward_x, p.forward_z)
}

/// Offset a position laterally from the road center using the perpendicular of the forward direction.
/// Positive lateral = right side of road, negative = left side.
pub fn offset_position(pos_x: f32, pos_z: f32, forward_x: f32, forward_z: f32, lateral: f32) -> (f32, f32) {
    let perp_x = -forward_z;
    let perp_z = forward_x;
    (pos_x + perp_x * lateral, pos_z + perp_z * lateral)
}

pub struct TerrainConfig {
    pub half_width: f32,
    pub cross_sections: usize,
    pub falloff: f32,
    pub elevation_scale: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            half_width: 300.0,
            cross_sections: 48,
            falloff: 0.08,
            elevation_scale: 1.0,
        }
    }
}

pub fn generate_terrain(points: &[RoutePoint], config: &TerrainConfig) -> CpuMesh {
    let cols = config.cross_sections * 2 + 1;
    let rows = points.len();
    let mut vertices = Vec::with_capacity(rows * cols);

    for point in points {
        let center_y = point.elevation_m as f32 * config.elevation_scale;
        let perp_x = -point.forward_z;
        let perp_z = point.forward_x;

        for j in 0..cols {
            let t = (j as f32 / (cols - 1) as f32) * 2.0 - 1.0;
            let lateral = t * config.half_width;
            let dist = t.abs();

            // Gentle slope near road, then gradual falloff further out
            let near_falloff = dist.powf(1.5) * config.half_width * config.falloff;
            let far_flatten = dist.powf(3.0) * config.half_width * 0.02;
            let y = center_y - near_falloff - far_flatten;

            let x = point.pos_x + perp_x * lateral;
            let z = point.pos_z + perp_z * lateral;

            let u = j as f32 / (cols - 1) as f32;
            let v = point.distance_m as f32 / 100.0;

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [0.0, 1.0, 0.0],
                uv: [u, v],
            });
        }
    }

    let mut indices = Vec::with_capacity((rows - 1) * (cols - 1) * 6);
    for i in 0..(rows - 1) {
        for j in 0..(cols - 1) {
            let tl = (i * cols + j) as u32;
            let tr = tl + 1;
            let bl = ((i + 1) * cols + j) as u32;
            let br = bl + 1;

            indices.push(tl);
            indices.push(bl);
            indices.push(tr);
            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    // Recompute normals
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
        let mut n = n.normalize_or_zero();
        // Ensure normals always point upward — on tight curves, inner triangles
        // can invert, flipping the computed normal downward. Force Y positive
        // so these triangles still shade correctly as ground.
        if n.y < 0.0 {
            n = -n;
        }
        vertices[i].normal = [n.x, n.y, n.z];
    }

    CpuMesh { vertices, indices }
}

/// A large flat ground plane that extends far beyond the terrain to prevent
/// seeing sky through terrain edges on hills.
/// Computes AABB from all route point positions and extends by half_extent.
pub fn generate_ground_plane(points: &[RoutePoint], half_extent: f32) -> CpuMesh {
    let min_elev = points
        .iter()
        .map(|p| p.elevation_m)
        .fold(f64::MAX, f64::min) as f32
        - 2.0;

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for p in points {
        min_x = min_x.min(p.pos_x);
        max_x = max_x.max(p.pos_x);
        min_z = min_z.min(p.pos_z);
        max_z = max_z.max(p.pos_z);
    }

    min_x -= half_extent;
    max_x += half_extent;
    min_z -= half_extent;
    max_z += half_extent;

    let vertices = vec![
        Vertex {
            position: [min_x, min_elev, min_z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [max_x, min_elev, min_z],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [min_x, min_elev, max_z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [max_x, min_elev, max_z],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        },
    ];

    let indices = vec![0, 2, 1, 1, 2, 3];
    CpuMesh { vertices, indices }
}
