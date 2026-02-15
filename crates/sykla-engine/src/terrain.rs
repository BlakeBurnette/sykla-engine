use crate::dem::{DemTile, GeoOrigin};
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
    /// Road banking angle in radians (positive = right side higher).
    pub banking: f32,
    /// Original (pre-stitch) world X for DEM sampling. Falls back to pos_x when not set.
    pub geo_x: f32,
    /// Original (pre-stitch) world Z for DEM sampling. Falls back to pos_z when not set.
    pub geo_z: f32,
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
            banking: 0.0,
            geo_x: 0.0,
            geo_z: 0.0,
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

/// Like `interpolate_point` but also returns the interpolated geo coordinates (geo_x, geo_z)
/// for DEM sampling. Returns (pos_x, pos_z, elevation, forward_x, forward_z, geo_x, geo_z).
pub fn interpolate_point_geo(points: &[RoutePoint], dist: f64) -> (f32, f32, f32, f32, f32, f32, f32) {
    if points.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    }
    if dist <= points[0].distance_m {
        let p = &points[0];
        return (p.pos_x, p.pos_z, p.elevation_m as f32, p.forward_x, p.forward_z, p.geo_x, p.geo_z);
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
            let fx = prev.forward_x + t * (curr.forward_x - prev.forward_x);
            let fz = prev.forward_z + t * (curr.forward_z - prev.forward_z);
            let len = (fx * fx + fz * fz).sqrt().max(0.0001);
            let gx = prev.geo_x + t * (curr.geo_x - prev.geo_x);
            let gz = prev.geo_z + t * (curr.geo_z - prev.geo_z);
            return (px, pz, elev, fx / len, fz / len, gx, gz);
        }
    }
    let p = points.last().unwrap();
    (p.pos_x, p.pos_z, p.elevation_m as f32, p.forward_x, p.forward_z, p.geo_x, p.geo_z)
}

/// Offset a position laterally from the road center using the perpendicular of the forward direction.
/// Positive lateral = right side of road, negative = left side.
pub fn offset_position(pos_x: f32, pos_z: f32, forward_x: f32, forward_z: f32, lateral: f32) -> (f32, f32) {
    let perp_x = -forward_z;
    let perp_z = forward_x;
    (pos_x + perp_x * lateral, pos_z + perp_z * lateral)
}

/// Real DEM elevation source for terrain generation.
pub struct DemTerrainSource {
    pub tile: DemTile,
    pub origin: GeoOrigin,
}

impl DemTerrainSource {
    /// Sample DEM elevation using geo coordinates offset from a route point.
    /// `geo_x, geo_z` are the original GPS-projected coords of the route point.
    /// `lateral_offset` is added via the forward direction's perpendicular.
    pub fn sample_at(&self, geo_x: f32, geo_z: f32, forward_x: f32, forward_z: f32, lateral: f32) -> Option<f32> {
        let perp_x = -forward_z;
        let perp_z = forward_x;
        let sample_x = geo_x + perp_x * lateral;
        let sample_z = geo_z + perp_z * lateral;
        self.tile.sample_world(&self.origin, sample_x, sample_z)
    }
}

pub struct TerrainConfig {
    pub half_width: f32,
    pub cross_sections: usize,
    pub falloff: f32,
    pub elevation_scale: f32,
    /// Noise amplitude multiplier — 1.0 for flat trails, 30-50 for mountain terrain.
    pub noise_amplitude: f32,
    /// Optional DEM data for real elevation terrain.
    pub dem: Option<DemTerrainSource>,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            half_width: 300.0,
            cross_sections: 48,
            falloff: 0.08,
            elevation_scale: 1.0,
            noise_amplitude: 1.0,
            dem: None,
        }
    }
}

pub fn generate_terrain(points: &[RoutePoint], config: &TerrainConfig) -> CpuMesh {
    let cols = config.cross_sections * 2 + 1;
    let rows = points.len();
    let mut vertices = Vec::with_capacity(rows * cols);

    let mountain_mode = config.noise_amplitude > 2.0;
    let has_dem = config.dem.is_some();


    for point in points {
        let center_y = point.elevation_m as f32 * config.elevation_scale;
        let perp_x = -point.forward_z;
        let perp_z = point.forward_x;

        // Mountain terrain: which side is "cliff wall" vs "valley" at this road position.
        // Low-frequency noise (~500m wavelength) so it changes roughly once per hairpin.
        let tilt = if mountain_mode && !has_dem {
            simple_noise(point.pos_x * 0.002, point.pos_z * 0.002)
        } else {
            0.0
        };

        for j in 0..cols {
            let t = (j as f32 / (cols - 1) as f32) * 2.0 - 1.0;
            let lateral = t * config.half_width;
            let dist = t.abs();

            let x = point.pos_x + perp_x * lateral;
            let z = point.pos_z + perp_z * lateral;

            let mut y;

            if let Some(dem) = &config.dem {
                // DEM path: real elevation from heightmap
                // Use original GPS coordinates (geo_x/geo_z) for DEM lookup to avoid
                // stitch translation errors. The lateral offset is the same since
                // stitching only translates, never rotates.
                let geo_x = point.geo_x + perp_x * lateral;
                let geo_z = point.geo_z + perp_z * lateral;
                let dist_from_center = dist * config.half_width;
                let road_y = center_y;

                if let Some(dem_y) = dem.tile.sample_world(&dem.origin, geo_x, geo_z) {
                    // Blend: road elevation at center, DEM elevation at edges
                    let blend = smoothstep_f32((dist_from_center - 5.0) / 10.0);
                    y = road_y + blend * (dem_y - road_y);
                    // Add small high-freq noise for texture (DEM is 30m grid, looks blocky otherwise)
                    y += simple_noise(x * 0.06, z * 0.06) * 2.0 * blend;
                } else {
                    // Out of DEM bounds — fall back to road elevation with gentle slope
                    y = road_y;
                }
            } else {
                // Procedural path (unchanged)
                let near_falloff = dist.powf(1.5) * config.half_width * config.falloff;
                let far_flatten = dist.powf(3.0) * config.half_width * 0.02;
                y = center_y - near_falloff - far_flatten;

                // Mountain terrain: asymmetric slope
                if mountain_mode {
                    y += t * tilt * dist.powf(0.7) * config.half_width * 0.4;
                }

                // Procedural terrain noise
                let n1 = simple_noise(x * 0.015, z * 0.015); // ~67m wavelength
                let n2 = simple_noise(x * 0.06, z * 0.06);   // ~17m wavelength
                let base_undulation = n1 * 0.7 + n2 * 0.2;

                if mountain_mode {
                    let n0 = simple_noise(x * 0.004, z * 0.004); // ~250m mountain ridges
                    let mountain_noise = n0 * 0.6 + n1 * 0.3 + n2 * 0.1;
                    let undulation = base_undulation + mountain_noise * config.noise_amplitude;
                    let undulation_factor = smoothstep_f32((dist - 0.05) / 0.15);
                    y += undulation * undulation_factor;
                } else {
                    let undulation_factor = smoothstep_f32((dist - 0.30) / 0.25);
                    y += base_undulation * undulation_factor;
                }
            }

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

    // --- Edge skirts: drop vertical triangles at all mesh edges to seal gaps ---
    let skirt_depth = 30.0_f32; // meters below edge — covers any gap to ground plane

    // Left edge (j=0) and right edge (j=cols-1)
    for i in 0..(rows - 1) {
        for &j in &[0, cols - 1] {
            let top_idx = (i * cols + j) as u32;
            let bot_idx = ((i + 1) * cols + j) as u32;

            // Copy vertex data to avoid borrow conflicts
            let vt = vertices[top_idx as usize];
            let vb = vertices[bot_idx as usize];

            let base_len = vertices.len() as u32;
            vertices.push(Vertex {
                position: [vt.position[0], vt.position[1] - skirt_depth, vt.position[2]],
                normal: vt.normal,
                uv: vt.uv,
            });
            vertices.push(Vertex {
                position: [vb.position[0], vb.position[1] - skirt_depth, vb.position[2]],
                normal: vb.normal,
                uv: vb.uv,
            });

            if j == 0 {
                indices.push(top_idx);
                indices.push(base_len);
                indices.push(bot_idx);
                indices.push(bot_idx);
                indices.push(base_len);
                indices.push(base_len + 1);
            } else {
                indices.push(top_idx);
                indices.push(bot_idx);
                indices.push(base_len);
                indices.push(bot_idx);
                indices.push(base_len + 1);
                indices.push(base_len);
            }
        }
    }

    // Front edge (i=0) and back edge (i=rows-1)
    for &i in &[0, rows - 1] {
        for j in 0..(cols - 1) {
            let left_idx = (i * cols + j) as u32;
            let right_idx = (i * cols + j + 1) as u32;

            let vl = vertices[left_idx as usize];
            let vr = vertices[right_idx as usize];

            let base_len = vertices.len() as u32;
            vertices.push(Vertex {
                position: [vl.position[0], vl.position[1] - skirt_depth, vl.position[2]],
                normal: vl.normal,
                uv: vl.uv,
            });
            vertices.push(Vertex {
                position: [vr.position[0], vr.position[1] - skirt_depth, vr.position[2]],
                normal: vr.normal,
                uv: vr.uv,
            });

            if i == 0 {
                indices.push(left_idx);
                indices.push(right_idx);
                indices.push(base_len);
                indices.push(right_idx);
                indices.push(base_len + 1);
                indices.push(base_len);
            } else {
                indices.push(left_idx);
                indices.push(base_len);
                indices.push(right_idx);
                indices.push(right_idx);
                indices.push(base_len);
                indices.push(base_len + 1);
            }
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

// --- Procedural noise helpers for terrain undulation ---

/// Smooth Hermite interpolation: 0 for t<=0, 1 for t>=1, smooth in between.
pub fn smoothstep_f32(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Compute procedural terrain elevation at a lateral offset from the road center.
/// Matches the terrain mesh generation exactly (non-DEM path).
pub fn procedural_terrain_y(
    center_y: f32,
    lateral: f32,
    half_width: f32,
    falloff: f32,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let t = lateral / half_width;
    let dist = t.abs();

    let near_falloff = dist.powf(1.5) * half_width * falloff;
    let far_flatten = dist.powf(3.0) * half_width * 0.02;
    let mut y = center_y - near_falloff - far_flatten;

    // Matching noise (non-mountain path)
    let n1 = simple_noise(world_x * 0.015, world_z * 0.015);
    let n2 = simple_noise(world_x * 0.06, world_z * 0.06);
    let base_undulation = n1 * 0.7 + n2 * 0.2;
    let undulation_factor = smoothstep_f32((dist - 0.30) / 0.25);
    y += base_undulation * undulation_factor;

    y
}

/// Deterministic grid hash returning a float in -1.0..1.0
fn grid_hash(ix: i32, iz: i32) -> f32 {
    let a = (ix as u32).wrapping_mul(2654435761);
    let b = (iz as u32).wrapping_mul(2246822519);
    let h = a.wrapping_add(b);
    (h % 10000) as f32 / 5000.0 - 1.0
}

/// Value noise with smooth interpolation. Returns -1.0..1.0 range.
pub fn simple_noise(x: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - x.floor();
    let fz = z - z.floor();

    // Smoothstep interpolation
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);

    let v00 = grid_hash(ix, iz);
    let v10 = grid_hash(ix + 1, iz);
    let v01 = grid_hash(ix, iz + 1);
    let v11 = grid_hash(ix + 1, iz + 1);

    let a = v00 + (v10 - v00) * ux;
    let b = v01 + (v11 - v01) * ux;
    a + (b - a) * uz
}
