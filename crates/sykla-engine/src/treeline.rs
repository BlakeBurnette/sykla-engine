use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::{interpolate_point_geo, RoutePoint, DemTerrainSource};
use crate::vegetation::InstanceData;

pub struct TreelineConfig {
    pub close_distance: f32,  // lateral offset for close strip (m)
    pub far_distance: f32,    // lateral offset for far strip (m)
    pub close_spacing: f32,   // spacing along route for close strip (m)
    pub far_spacing: f32,     // spacing along route for far strip (m)
    pub strip_height: f32,    // billboard height (m)
}

impl Default for TreelineConfig {
    fn default() -> Self {
        Self {
            close_distance: 200.0,
            far_distance: 500.0,
            close_spacing: 20.0,
            far_spacing: 40.0,
            strip_height: 15.0,
        }
    }
}

/// A flat vertical quad used as a billboard tree strip.
/// Width=1, height=1 centered at Y=0 (instances offset Y to ground + half-height).
pub fn generate_billboard_quad() -> CpuMesh {
    let hw = 0.5_f32;
    let vertices = vec![
        Vertex { position: [-hw, -0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
        Vertex { position: [ hw, -0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
        Vertex { position: [ hw,  0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
        Vertex { position: [-hw,  0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
        // Back face (opposite normal) so visible from both sides
        Vertex { position: [ hw, -0.5, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
        Vertex { position: [-hw, -0.5, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
        Vertex { position: [-hw,  0.5, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
        Vertex { position: [ hw,  0.5, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
    ];
    let indices = vec![
        0, 1, 2, 0, 2, 3, // front
        4, 5, 6, 4, 6, 7, // back
    ];
    CpuMesh { vertices, indices }
}

/// Generates billboard tree strip instances at two distance bands from the route.
/// Returns (close_strip, far_strip) as Vec<InstanceData>.
pub fn generate_treeline_strips(
    points: &[RoutePoint],
    config: &TreelineConfig,
    dem: Option<&DemTerrainSource>,
) -> (Vec<InstanceData>, Vec<InstanceData>) {
    if points.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let route_len = points.last().unwrap().distance_m;
    let mut close_instances = Vec::new();
    let mut far_instances = Vec::new();

    // Close strip — both sides of route
    generate_strip(
        points,
        route_len,
        config.close_distance,
        config.close_spacing,
        config.strip_height,
        dem,
        &mut close_instances,
    );

    // Far strip
    generate_strip(
        points,
        route_len,
        config.far_distance,
        config.far_spacing,
        config.strip_height * 1.1, // slightly taller at distance
        dem,
        &mut far_instances,
    );

    (close_instances, far_instances)
}

fn generate_strip(
    points: &[RoutePoint],
    route_len: f64,
    lateral_distance: f32,
    spacing: f32,
    height: f32,
    dem: Option<&DemTerrainSource>,
    out: &mut Vec<InstanceData>,
) {
    let num_steps = (route_len as f32 / spacing).ceil() as usize;
    let hash_seed = (lateral_distance * 73.1) as u32;

    for i in 0..num_steps {
        let dist = i as f64 * spacing as f64;
        let (cx, cz, elev, _fx, fz, gx, gz) = interpolate_point_geo(points, dist);

        // Perpendicular direction
        let perp_x = -fz;
        let perp_z = _fx;

        // Place on both sides with slight random offset
        for side in [-1.0_f32, 1.0] {
            let h = pos_hash(i as u32, hash_seed.wrapping_add(if side > 0.0 { 0 } else { 7919 }));

            // Random lateral jitter (±20%)
            let jitter = 1.0 + ((h % 200) as f32 / 1000.0 - 0.1);
            let lat = lateral_distance * jitter * side;

            let x = cx + perp_x * lat;
            let z = cz + perp_z * lat;

            // Sample elevation from DEM if available, otherwise use route elevation
            // Use geo coordinates (original GPS positions) for DEM lookup
            let y = if let Some(dem_src) = dem {
                dem_src.sample_at(gx, gz, _fx, fz, lat).unwrap_or(elev)
            } else {
                elev
            };

            // Scale = height with random variation
            let scale = height * (0.85 + (h % 150) as f32 / 500.0);

            out.push(InstanceData {
                position: [x, y + scale * 0.5, z], // center at half-height
                scale,
            });
        }
    }
}

fn pos_hash(a: u32, b: u32) -> u32 {
    a.wrapping_mul(2654435761).wrapping_add(b.wrapping_mul(2246822519))
}
