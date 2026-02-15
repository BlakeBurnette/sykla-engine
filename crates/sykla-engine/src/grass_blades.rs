use crate::mesh::{CpuMesh, Vertex};
use crate::terrain::{interpolate_point_geo, offset_position, procedural_terrain_y, DemTerrainSource, RoutePoint};
use bytemuck::{Pod, Zeroable};

/// Maximum grass blades per frame.
pub const MAX_GRASS_BLADES: usize = if cfg!(target_arch = "wasm32") {
    150_000
} else {
    400_000
};

/// Per-blade instance data (32 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GrassBladeInstance {
    pub position: [f32; 3],
    pub height: f32,
    pub rotation_y: f32,
    pub lean_angle: f32,
    pub lean_direction: f32,
    pub color_variation: f32,
}

impl GrassBladeInstance {
    const ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        3 => Float32x3,
        4 => Float32,
        5 => Float32,
        6 => Float32,
        7 => Float32,
        8 => Float32,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GrassBladeInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRS,
        }
    }
}

/// Grass tuft mesh — 3 intersecting tapered strips at 60° angles ("star" pattern).
/// Each strip has 7 vertices and 5 triangles = 21 vertices, 15 triangles total.
/// This gives dense ground coverage from any viewing angle.
/// Height = 1.0 (scaled by instance height).
pub fn create_blade_mesh() -> CpuMesh {
    let w = 0.045; // half-width at base (~90mm per strip)

    // 3 strips at 0°, 60°, 120° rotation around Y
    let angles = [0.0_f32, std::f32::consts::FRAC_PI_3 * 2.0, std::f32::consts::FRAC_PI_3 * 4.0];
    let mut vertices = Vec::with_capacity(21);
    let mut indices = Vec::with_capacity(45);

    for &angle in &angles {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let base = vertices.len() as u32;

        // Heights and width multipliers for each level
        let levels: [(f32, f32); 4] = [
            (0.0, 1.0),    // base
            (0.33, 0.75),  // mid
            (0.67, 0.40),  // upper
            (1.0, 0.0),    // tip (single vertex)
        ];

        for (i, &(y, wm)) in levels.iter().enumerate() {
            if i < 3 {
                // Two vertices per level (left and right)
                let lx = -w * wm * cos_a;
                let lz = -w * wm * sin_a;
                let rx = w * wm * cos_a;
                let rz = w * wm * sin_a;
                vertices.push(Vertex { position: [lx, y, lz], normal: [-sin_a, 0.0, cos_a], uv: [0.0, y] });
                vertices.push(Vertex { position: [rx, y, rz], normal: [-sin_a, 0.0, cos_a], uv: [1.0, y] });
            } else {
                // Single tip vertex
                vertices.push(Vertex { position: [0.0, y, 0.0], normal: [-sin_a, 0.0, cos_a], uv: [0.5, y] });
            }
        }

        // 5 triangles per strip: 2 base + 2 mid + 1 tip
        indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base + 1, base + 2, base + 3,
            base + 2, base + 4, base + 3,
            base + 3, base + 4, base + 5,
            base + 4, base + 6, base + 5,
        ]);
    }

    CpuMesh { vertices, indices }
}

/// Deterministic hash for blade placement — matches shader hash() function.
fn blade_hash(x: f32, z: f32) -> f32 {
    let h = x * 127.1 + z * 311.7;
    (h.sin() * 43758.5453).fract().abs()
}

/// Grass blade placement configuration.
pub struct GrassBladeConfig {
    pub blade_height: f32,
    pub blade_height_variation: f32,
    pub density: f32,
    pub color_base: [f32; 3],
    pub color_tip: [f32; 3],
    pub wind_strength: f32,
    pub fade_start: f32,
    pub fade_end: f32,
    pub lateral_extent: f32,
    pub road_half_width: f32,
}

impl GrassBladeConfig {
    /// Returns blade config for the given biome type, or None if the biome has no grass.
    /// Biome types: 1=piedmont, 2=alpine, 3=desert (no grass), 4=coastal, 5=forest
    pub fn for_biome(biome_type: f32) -> Option<Self> {
        if biome_type < 0.5 || (biome_type > 2.5 && biome_type < 3.5) {
            return None;
        }

        Some(if biome_type < 1.5 {
            // Piedmont
            Self {
                blade_height: 0.45,
                blade_height_variation: 0.20,
                density: 0.88,
                color_base: [0.18, 0.30, 0.08],
                color_tip: [0.35, 0.52, 0.18],
                wind_strength: 0.4,
                fade_start: 35.0,
                fade_end: 55.0,
                lateral_extent: 35.0,
                road_half_width: 5.5,
            }
        } else if biome_type < 2.5 {
            // Alpine
            Self {
                blade_height: 0.50,
                blade_height_variation: 0.22,
                density: 0.80,
                color_base: [0.16, 0.28, 0.06],
                color_tip: [0.30, 0.48, 0.15],
                wind_strength: 0.6,
                fade_start: 35.0,
                fade_end: 55.0,
                lateral_extent: 35.0,
                road_half_width: 5.5,
            }
        } else if biome_type < 4.5 {
            // Coastal
            Self {
                blade_height: 0.55,
                blade_height_variation: 0.25,
                density: 0.85,
                color_base: [0.22, 0.34, 0.10],
                color_tip: [0.45, 0.58, 0.25],
                wind_strength: 0.7,
                fade_start: 35.0,
                fade_end: 55.0,
                lateral_extent: 35.0,
                road_half_width: 5.5,
            }
        } else {
            // Forest
            Self {
                blade_height: 0.50,
                blade_height_variation: 0.20,
                density: 0.90,
                color_base: [0.14, 0.25, 0.06],
                color_tip: [0.28, 0.45, 0.14],
                wind_strength: 0.25,
                fade_start: 35.0,
                fade_end: 55.0,
                lateral_extent: 35.0,
                road_half_width: 5.5,
            }
        })
    }
}

/// Generate grass blade instances around the camera position along the route.
pub fn generate_blade_instances(
    points: &[RoutePoint],
    config: &GrassBladeConfig,
    camera_dist: f64,
    dem: Option<&DemTerrainSource>,
    terrain_half_width: f32,
    terrain_falloff: f32,
) -> Vec<GrassBladeInstance> {
    let mut blades = Vec::with_capacity(MAX_GRASS_BLADES);
    if points.is_empty() {
        return blades;
    }

    let route_length = points.last().map(|p| p.distance_m).unwrap_or(0.0);
    let window = 45.0_f64;
    let start_dist = (camera_dist - window).max(0.0);
    let end_dist = (camera_dist + window).min(route_length);

    let longitudinal_step = 0.12_f64;
    let mut dist = start_dist;

    while dist < end_dist && blades.len() < MAX_GRASS_BLADES {
        let (px, pz, elev, fx, fz, gx, gz) = interpolate_point_geo(points, dist);

        let mut lateral = -config.lateral_extent;
        while lateral < config.lateral_extent && blades.len() < MAX_GRASS_BLADES {
            let abs_lat = lateral.abs();

            // Skip road surface
            if abs_lat < config.road_half_width {
                lateral += 0.3;
                continue;
            }

            // Variable spacing: denser near road, sparser far away
            let spacing = if abs_lat < 10.0 {
                0.08
            } else if abs_lat < 20.0 {
                0.14
            } else {
                0.25
            };

            // Compute world position for hashing
            let (world_x, world_z) = offset_position(px, pz, fx, fz, lateral);

            // Hash-based density culling (deterministic — no flicker)
            let h = blade_hash(world_x * 3.7, world_z * 3.7);
            if h > config.density {
                lateral += spacing;
                continue;
            }

            // Jitter position to break grid pattern
            let jx = (blade_hash(world_x * 17.3, world_z * 29.1) - 0.5) * spacing * 0.8;
            let jz = (blade_hash(world_x * 31.7, world_z * 13.3) - 0.5) * spacing * 0.8;
            let jittered_x = world_x + jx;
            let jittered_z = world_z + jz;

            // Get elevation from DEM or procedural terrain
            let y = if let Some(dem) = dem {
                dem.sample_at(gx, gz, fx, fz, lateral).unwrap_or(elev)
            } else {
                procedural_terrain_y(elev, lateral, terrain_half_width, terrain_falloff, world_x, world_z)
            };

            // Per-blade variation from hash
            let h2 = blade_hash(world_x * 7.3, world_z * 11.1);
            let h3 = blade_hash(world_x * 13.7, world_z * 5.3);
            let h4 = blade_hash(world_x * 19.1, world_z * 23.7);

            // Blade type encoding via color_variation:
            // 0.00-0.69: normal grass (70%)
            // 0.70-0.84: weed — taller, darker green
            // 0.85-0.94: dead/dry grass — straw yellow-brown
            // 0.95-1.00: fallen leaf — flat, brown/red/yellow
            let type_hash = blade_hash(world_x * 23.1, world_z * 37.7);
            let color_variation = type_hash;

            // Adjust height and lean based on blade type
            let is_weed = type_hash >= 0.70 && type_hash < 0.85;
            let is_leaf = type_hash >= 0.95;
            let height_mult = if is_weed { 1.5 } else { 1.0 };
            let height =
                (config.blade_height + (h2 - 0.5) * 2.0 * config.blade_height_variation) * height_mult;
            let rotation_y = h3 * std::f32::consts::TAU;
            // Fallen leaves lie flat (high lean angle)
            let lean_angle = if is_leaf { 1.2 } else { h4 * 0.25 };
            let lean_direction = h2 * std::f32::consts::TAU;

            blades.push(GrassBladeInstance {
                position: [jittered_x, y, jittered_z],
                height,
                rotation_y,
                lean_angle,
                lean_direction,
                color_variation,
            });

            lateral += spacing;
        }

        dist += longitudinal_step;
    }

    blades
}
