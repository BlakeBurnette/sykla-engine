use std::sync::Arc;
use std::time::Instant;

use sykla_engine::camera::{Camera, CameraMode};
use sykla_engine::dem::{DemTile, GeoOrigin};
use sykla_engine::glam::Vec3;
use sykla_engine::hud::{build_hud, build_selector_hud, camera_button_rects};
use sykla_engine::mesh::GpuMesh;
use sykla_engine::physics::{PhysicsParams, PhysicsState, update_physics};
use sykla_engine::cyclist::{
    CyclistInstanceData, NpcCyclist, load_cyclist_glb, spawn_npc_cyclists, update_cyclists,
};
use sykla_engine::mountains::{generate_mountain_ring, generate_mountain_ring_with_radius};
use sykla_engine::skeleton::SkeletonSystem;
use sykla_engine::grass;
use sykla_engine::grass_blades;
use sykla_engine::jersey;
use sykla_engine::renderer::{AnimatedDrawCall, CyclistMeshPart, CyclistModel, CyclistPipeline, DrawCall, GrassBladeDrawCall, GrassShellDraw, InstancedDrawCall, Renderer, TreeModel};
use sykla_engine::road::{generate_roads_by_surface, RoadConfig};
use sykla_engine::routes::{self, RoadMarkings};
use sykla_engine::structures;
use sykla_engine::terrain::{generate_ground_plane, generate_terrain, interpolate_point, DemTerrainSource, RoutePoint, SurfaceType};
use sykla_engine::vegetation::{
    generate_bush, generate_canopy, generate_pine_canopy, generate_pine_trunk, generate_trunk,
    place_vegetation,
};
use sykla_engine::treeline::{generate_billboard_quad, generate_treeline_strips, TreelineConfig};
use sykla_engine::water::{detect_water_features, generate_water_mesh};
use sykla_engine::wildlife::{
    generate_bird, generate_deer, generate_egret, generate_goose, generate_small_bird,
    generate_squirrel, generate_turtle, place_wildlife, AnimatedInstanceData,
};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(PartialEq)]
enum AppState {
    Selecting,
    Riding,
}

struct App {
    state: AppState,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    camera: Camera,
    draws: Vec<DrawCall>,
    road_draws: Vec<DrawCall>,
    instanced_draws: Vec<InstancedDrawCall>,
    animated_draws: Vec<AnimatedDrawCall>,
    water_draws: Vec<DrawCall>,
    cyclist_models: Vec<CyclistModel>,
    emissive_instanced_draws: Vec<InstancedDrawCall>,
    tree_models: Vec<TreeModel>,
    grass_shell_draws: Vec<GrassShellDraw>,
    grass_blade_draws: Vec<GrassBladeDrawCall>,
    grass_blade_config: Option<grass_blades::GrassBladeConfig>,
    grass_blade_last_dist: f32,
    dem_for_grass: Option<DemTerrainSource>,
    terrain_half_width: f32,
    terrain_falloff: f32,
    // CPU copies for per-frame position updates
    flying_cpu: Vec<Vec<AnimatedInstanceData>>,
    cyclist_cpu: Vec<CyclistInstanceData>,
    npc_cyclists: Vec<NpcCyclist>,
    skeleton_system: SkeletonSystem,
    start_time: Instant,
    prev_t: f32,
    route_length: f32,
    route_points: Vec<RoutePoint>,
    geo_origin: Option<GeoOrigin>,
    physics_state: PhysicsState,
    physics_params: PhysicsParams,
    current_route: usize,
    current_key: &'static str,
    camera_mode: CameraMode,
    cursor_pos: (f32, f32),
    sky_color: [f32; 3],
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::Selecting,
            window: None,
            renderer: None,
            camera: Camera::new(16.0 / 9.0),
            draws: Vec::new(),
            road_draws: Vec::new(),
            instanced_draws: Vec::new(),
            animated_draws: Vec::new(),
            water_draws: Vec::new(),
            cyclist_models: Vec::new(),
            emissive_instanced_draws: Vec::new(),
            tree_models: Vec::new(),
            grass_shell_draws: Vec::new(),
            grass_blade_draws: Vec::new(),
            grass_blade_config: None,
            grass_blade_last_dist: -999.0,
            dem_for_grass: None,
            terrain_half_width: 300.0,
            terrain_falloff: 0.08,
            flying_cpu: Vec::new(),
            cyclist_cpu: Vec::new(),
            npc_cyclists: Vec::new(),
            skeleton_system: SkeletonSystem::new(32),
            start_time: Instant::now(),
            prev_t: 0.0,
            route_length: 0.0,
            route_points: Vec::new(),
            geo_origin: None,
            physics_state: PhysicsState::default(),
            physics_params: PhysicsParams::default(),
            current_route: 0,
            current_key: "att",
            camera_mode: CameraMode::ThirdPersonClose,
            cursor_pos: (0.0, 0.0),
            sky_color: [0.52, 0.70, 0.82],
        }
    }

    fn load_route(&mut self, index: usize) {
        let cat = routes::catalog();
        let key = cat[index].key;
        let (points, geo_origin) = routes::generate_route(key);
        self.route_points = points;
        self.geo_origin = geo_origin;
        self.route_length = self.route_points.last().map(|p| p.distance_m as f32).unwrap_or(0.0);
        self.current_route = index;
        self.current_key = key;
        self.physics_state = PhysicsState::default();
        log::info!("Loaded route: {} ({:.1} km)", cat[index].name, self.route_length / 1000.0);
    }

    fn build_world(&mut self) {
        let style = routes::route_style(self.current_key);

        // Update shadow VP light direction (mutable access)
        if let Some(renderer) = &mut self.renderer {
            renderer.light_dir = Vec3::new(
                style.light_direction[0],
                style.light_direction[1],
                style.light_direction[2],
            ).normalize();
        }

        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        // --- Terrain, ground plane, and road ---
        let dem_source = match (style.dem_data, &self.geo_origin) {
            (Some(bytes), Some(origin)) => {
                DemTile::from_bytes(bytes).map(|tile| DemTerrainSource { tile, origin: origin.clone() })
            }
            _ => None,
        };

        // Resample route elevations from DEM — always use DEM when available.
        // The hardcoded elevation profiles are approximations; DEM (SRTM) is authoritative.
        // Use geo_x/geo_z (original GPS coordinates) for DEM lookup, not pos_x/pos_z
        // (which may be translated by stitch_segments).
        if let Some(dem) = &dem_source {
            for point in &mut self.route_points {
                let sample_x = if point.geo_x != 0.0 || point.geo_z != 0.0 { point.geo_x } else { point.pos_x };
                let sample_z = if point.geo_x != 0.0 || point.geo_z != 0.0 { point.geo_z } else { point.pos_z };
                if let Some(dem_elev) = dem.tile.sample_world(&dem.origin, sample_x, sample_z) {
                    point.elevation_m = dem_elev as f64;
                }
            }
        }

        let is_dem = dem_source.is_some();
        let terrain_config = style.terrain_config(dem_source);
        self.terrain_half_width = terrain_config.half_width;
        self.terrain_falloff = terrain_config.falloff;
        let terrain_mesh = generate_terrain(&self.route_points, &terrain_config);
        let ground_half = if is_dem { 3000.0 } else { 1000.0 };
        let ground_mesh = generate_ground_plane(&self.route_points, ground_half);
        let road_segments = generate_roads_by_surface(&self.route_points, &RoadConfig::default());

        let terrain_gpu = GpuMesh::from_cpu(&renderer.device, &terrain_mesh);
        let ground_gpu = GpuMesh::from_cpu(&renderer.device, &ground_mesh);

        let terrain_mat = renderer.create_terrain_material(
            style.terrain_color,
            style.terrain_mid_color,
            style.terrain_high_color,
            style.elevation_zones,
            style.terrain_params,
        );
        let ground_mat = renderer.create_material(style.ground_color);

        // Set lighting, fog, and sky from route style
        renderer.set_light_direction_uniform(style.light_direction);
        renderer.set_light_color(style.light_color);
        renderer.set_light_ambient(style.light_ambient);
        renderer.set_fog_color(style.fog_color);
        self.sky_color = style.sky_color;

        self.draws = vec![
            DrawCall {
                mesh: ground_gpu,
                material_bind_group: ground_mat,
            },
            DrawCall {
                mesh: terrain_gpu,
                material_bind_group: terrain_mat,
            },
        ];

        // Mountains — background ring of peaks using terrain material
        if style.mountains {
            let mountain_mesh = if is_dem {
                generate_mountain_ring_with_radius(&self.route_points, 4000.0)
            } else {
                generate_mountain_ring(&self.route_points)
            };
            if !mountain_mesh.vertices.is_empty() {
                let mountain_gpu = GpuMesh::from_cpu(&renderer.device, &mountain_mesh);
                let mountain_mat = renderer.create_terrain_material(
                    style.terrain_color,
                    style.terrain_mid_color,
                    style.terrain_high_color,
                    style.elevation_zones,
                    [0.0; 4], // mountains don't use terrain splatting
                );
                self.draws.push(DrawCall {
                    mesh: mountain_gpu,
                    material_bind_group: mountain_mat,
                });
            }
        }

        // Road segments — separate draw list with depth-biased pipeline
        let has_markings = style.road_markings == RoadMarkings::European;
        self.road_draws = Vec::new();
        for (mesh, surface) in &road_segments {
            let color = match surface {
                SurfaceType::Paved => style.road_color,
                SurfaceType::Gravel => style.gravel_color,
            };
            let gpu = GpuMesh::from_cpu(&renderer.device, mesh);
            let mat = renderer.create_road_material(color, has_markings);
            self.road_draws.push(DrawCall {
                mesh: gpu,
                material_bind_group: mat,
            });
        }

        // Debug: log road mesh info
        let road_total_verts: usize = road_segments.iter().map(|(m, _)| m.vertices.len()).sum();
        let road_total_idx: usize = road_segments.iter().map(|(m, _)| m.indices.len()).sum();
        log::info!(
            "Road: {} segments, {} vertices, {} indices",
            road_segments.len(), road_total_verts, road_total_idx,
        );
        // Log path extent to verify winding
        if !self.route_points.is_empty() {
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;
            for p in &self.route_points {
                min_x = min_x.min(p.pos_x);
                max_x = max_x.max(p.pos_x);
                min_z = min_z.min(p.pos_z);
                max_z = max_z.max(p.pos_z);
            }
            log::info!(
                "  Path extent: X=[{:.0}..{:.0}] ({:.0}m), Z=[{:.0}..{:.0}] ({:.0}m)",
                min_x, max_x, max_x - min_x,
                min_z, max_z, max_z - min_z,
            );
        }

        // --- Grass shells (volumetric grass on terrain) ---
        let mut grass_shell_draws = Vec::new();
        if let Some(grass_params) = grass::GrassParams::for_biome(style.terrain_params[3]) {
            let grass_mesh = GpuMesh::from_cpu(&renderer.device, &terrain_mesh);
            let grass_mat = renderer.create_grass_material(&grass_params);
            grass_shell_draws.push(GrassShellDraw {
                mesh: grass_mesh,
                material_bind_group: grass_mat,
                num_shells: grass_params.num_shells,
            });
            log::info!("Grass: {} shells, height={:.3}m", grass_params.num_shells, grass_params.shell_height);
        }
        self.grass_shell_draws = grass_shell_draws;

        // --- Grass blades (instanced individual blades near camera) ---
        self.grass_blade_draws.clear();
        self.grass_blade_config = grass_blades::GrassBladeConfig::for_biome(style.terrain_params[3]);
        self.grass_blade_last_dist = -999.0;

        // Store DEM for per-frame blade placement
        self.dem_for_grass = match (style.dem_data, &self.geo_origin) {
            (Some(bytes), Some(origin)) => {
                DemTile::from_bytes(bytes).map(|tile| DemTerrainSource { tile, origin: origin.clone() })
            }
            _ => None,
        };

        if let Some(ref blade_config) = self.grass_blade_config {
            let blade_mesh = grass_blades::create_blade_mesh();
            let blade_gpu = GpuMesh::from_cpu(&renderer.device, &blade_mesh);
            let blade_mat = renderer.create_grass_blade_material(blade_config);
            let blade_buf = renderer.create_grass_blade_instance_buffer(grass_blades::MAX_GRASS_BLADES);

            // Generate initial blade instances at start
            let initial_blades = grass_blades::generate_blade_instances(
                &self.route_points,
                blade_config,
                0.0,
                self.dem_for_grass.as_ref(),
                self.terrain_half_width,
                self.terrain_falloff,
            );
            let blade_count = initial_blades.len() as u32;
            if !initial_blades.is_empty() {
                renderer.queue.write_buffer(
                    &blade_buf,
                    0,
                    bytemuck::cast_slice(&initial_blades),
                );
            }
            self.grass_blade_draws.push(GrassBladeDrawCall {
                mesh: blade_gpu,
                instance_buffer: blade_buf,
                instance_count: blade_count,
                material_bind_group: blade_mat,
            });
            log::info!("Grass blades: {} initial instances", blade_count);
        }

        // --- Vegetation (instanced) ---
        let placement = place_vegetation(
            &self.route_points, &style.vegetation, terrain_config.dem.as_ref(),
            terrain_config.half_width, terrain_config.falloff,
        );

        // Shared meshes
        let trunk_mesh = generate_trunk();
        let canopy_mesh = generate_canopy();
        let pine_trunk_mesh = generate_pine_trunk();
        let pine_canopy_mesh = generate_pine_canopy();
        let bush_mesh_cpu = generate_bush();

        // GPU meshes — reuse same mesh for mature + young (scale does the rest)
        let dec_trunk_gpu = GpuMesh::from_cpu(&renderer.device, &trunk_mesh);
        let dec_canopy_gpu = GpuMesh::from_cpu(&renderer.device, &canopy_mesh);
        let pine_trunk_gpu = GpuMesh::from_cpu(&renderer.device, &pine_trunk_mesh);
        let pine_canopy_gpu = GpuMesh::from_cpu(&renderer.device, &pine_canopy_mesh);
        let bush_gpu = GpuMesh::from_cpu(&renderer.device, &bush_mesh_cpu);

        // Young growth reuses same mesh, separate instance buffers
        let ydec_trunk_gpu = GpuMesh::from_cpu(&renderer.device, &trunk_mesh);
        let ydec_canopy_gpu = GpuMesh::from_cpu(&renderer.device, &canopy_mesh);
        let ypine_trunk_gpu = GpuMesh::from_cpu(&renderer.device, &pine_trunk_mesh);
        let ypine_canopy_gpu = GpuMesh::from_cpu(&renderer.device, &pine_canopy_mesh);

        // Instance buffers
        let dec_buf1 = renderer.create_instance_buffer(&placement.deciduous);
        let dec_buf2 = renderer.create_instance_buffer(&placement.deciduous);
        let pine_buf1 = renderer.create_instance_buffer(&placement.pine);
        let pine_buf2 = renderer.create_instance_buffer(&placement.pine);
        let bush_buf = renderer.create_instance_buffer(&placement.bush);
        let ydec_buf1 = renderer.create_instance_buffer(&placement.young_deciduous);
        let ydec_buf2 = renderer.create_instance_buffer(&placement.young_deciduous);
        let ypine_buf1 = renderer.create_instance_buffer(&placement.young_pine);
        let ypine_buf2 = renderer.create_instance_buffer(&placement.young_pine);

        let dec_count = placement.deciduous.len() as u32;
        let pine_count = placement.pine.len() as u32;
        let bush_count = placement.bush.len() as u32;
        let ydec_count = placement.young_deciduous.len() as u32;
        let ypine_count = placement.young_pine.len() as u32;

        // Materials — derived from route style
        let trunk_mat = renderer.create_material(style.trunk_color);
        let dec_canopy_mat = renderer.create_material(style.canopy_color);
        let pine_trunk_mat = renderer.create_material(style.trunk_color);
        // Winter routes: snow on pine canopies (detected by elevation_zones[0] < 90000)
        let is_winter = style.elevation_zones[0] < 90000.0;
        let pine_canopy_mat = if is_winter {
            renderer.create_snow_foliage_material(style.pine_color)
        } else {
            renderer.create_material(style.pine_color)
        };
        let bush_mat = renderer.create_material(style.bush_color);
        // Young growth: slightly lighter variants
        let young_trunk_mat = renderer.create_material([
            style.trunk_color[0] + 0.04,
            style.trunk_color[1] + 0.04,
            style.trunk_color[2] + 0.02,
            1.0,
        ]);
        let young_dec_canopy_mat = renderer.create_material([
            style.canopy_color[0] + 0.05,
            style.canopy_color[1] + 0.10,
            style.canopy_color[2] + 0.03,
            1.0,
        ]);
        let young_pine_color = [
            style.pine_color[0] + 0.04,
            style.pine_color[1] + 0.08,
            style.pine_color[2] + 0.02,
            1.0,
        ];
        let young_pine_canopy_mat = if is_winter {
            renderer.create_snow_foliage_material(young_pine_color)
        } else {
            renderer.create_material(young_pine_color)
        };

        self.instanced_draws = vec![
            InstancedDrawCall {
                mesh: dec_trunk_gpu,
                instance_buffer: dec_buf1,
                instance_count: dec_count,
                material_bind_group: trunk_mat.clone(),
            },
            InstancedDrawCall {
                mesh: dec_canopy_gpu,
                instance_buffer: dec_buf2,
                instance_count: dec_count,
                material_bind_group: dec_canopy_mat,
            },
            InstancedDrawCall {
                mesh: pine_trunk_gpu,
                instance_buffer: pine_buf1,
                instance_count: pine_count,
                material_bind_group: pine_trunk_mat.clone(),
            },
            InstancedDrawCall {
                mesh: pine_canopy_gpu,
                instance_buffer: pine_buf2,
                instance_count: pine_count,
                material_bind_group: pine_canopy_mat,
            },
            InstancedDrawCall {
                mesh: bush_gpu,
                instance_buffer: bush_buf,
                instance_count: bush_count,
                material_bind_group: bush_mat,
            },
            InstancedDrawCall {
                mesh: ydec_trunk_gpu,
                instance_buffer: ydec_buf1,
                instance_count: ydec_count,
                material_bind_group: young_trunk_mat.clone(),
            },
            InstancedDrawCall {
                mesh: ydec_canopy_gpu,
                instance_buffer: ydec_buf2,
                instance_count: ydec_count,
                material_bind_group: young_dec_canopy_mat,
            },
            InstancedDrawCall {
                mesh: ypine_trunk_gpu,
                instance_buffer: ypine_buf1,
                instance_count: ypine_count,
                material_bind_group: young_trunk_mat,
            },
            InstancedDrawCall {
                mesh: ypine_canopy_gpu,
                instance_buffer: ypine_buf2,
                instance_count: ypine_count,
                material_bind_group: young_pine_canopy_mat,
            },
        ];

        let veg_total = dec_count + pine_count + bush_count + ydec_count + ypine_count;
        log::info!(
            "Vegetation: {} oak, {} pine, {} bush, {} young oak, {} young pine = {} total",
            dec_count, pine_count, bush_count, ydec_count, ypine_count, veg_total
        );

        // --- Textured GLB trees (eastern species) ---
        self.tree_models.clear();
        if style.vegetation.use_eastern_species {
            let oak_parts = sykla_engine::trees::load_tree_species(renderer, include_bytes!("../../../assets/trees/oak.glb"));
            let pine_parts = sykla_engine::trees::load_tree_species(renderer, include_bytes!("../../../assets/trees/loblolly_pine.glb"));
            let redbud_parts = sykla_engine::trees::load_tree_species(renderer, include_bytes!("../../../assets/trees/redbud.glb"));

            for (parts, instances) in [
                (oak_parts, &placement.oaks),
                (pine_parts, &placement.loblolly_pines),
                (redbud_parts, &placement.redbuds),
            ] {
                if !instances.is_empty() {
                    let buf = renderer.create_tree_instance_buffer(instances);
                    self.tree_models.push(TreeModel {
                        parts,
                        instance_buffer: buf,
                        instance_count: instances.len() as u32,
                    });
                }
            }
            // Fallen trees use oak mesh
            if !placement.fallen_trees.is_empty() {
                let fallen_parts = sykla_engine::trees::load_tree_species(renderer, include_bytes!("../../../assets/trees/oak.glb"));
                let buf = renderer.create_tree_instance_buffer(&placement.fallen_trees);
                self.tree_models.push(TreeModel {
                    parts: fallen_parts,
                    instance_buffer: buf,
                    instance_count: placement.fallen_trees.len() as u32,
                });
            }
            log::info!(
                "Eastern trees: {} oaks, {} pines, {} redbuds, {} fallen",
                placement.oaks.len(), placement.loblolly_pines.len(),
                placement.redbuds.len(), placement.fallen_trees.len(),
            );
            // Debug: log GLB tree model details
            for (i, model) in self.tree_models.iter().enumerate() {
                log::info!(
                    "  TreeModel[{}]: {} parts, {} instances",
                    i, model.parts.len(), model.instance_count,
                );
                for (j, part) in model.parts.iter().enumerate() {
                    log::info!(
                        "    Part[{}]: {} indices",
                        j, part.mesh.num_indices,
                    );
                }
            }
        }

        // --- Treeline strips (distant billboard trees) ---
        // Skip when eastern species GLB trees are active — they provide enough coverage
        // and the billboard quads look like bare poles from most angles
        if !style.vegetation.use_eastern_species {
            let treeline_config = TreelineConfig::default();
            let (close_strip, far_strip) = generate_treeline_strips(
                &self.route_points,
                &treeline_config,
                None,
            );

            if !close_strip.is_empty() {
                let billboard_mesh = generate_billboard_quad();
                let billboard_gpu = GpuMesh::from_cpu(&renderer.device, &billboard_mesh);
                let close_mat = renderer.create_material(style.canopy_color);
                let close_buf = renderer.create_instance_buffer(&close_strip);
                self.instanced_draws.push(InstancedDrawCall {
                    mesh: billboard_gpu,
                    instance_buffer: close_buf,
                    instance_count: close_strip.len() as u32,
                    material_bind_group: close_mat,
                });
            }
            if !far_strip.is_empty() {
                let billboard_mesh = generate_billboard_quad();
                let billboard_gpu = GpuMesh::from_cpu(&renderer.device, &billboard_mesh);
                let far_color = [
                    style.canopy_color[0] * 0.7 + style.fog_color[0] * 0.3,
                    style.canopy_color[1] * 0.7 + style.fog_color[1] * 0.3,
                    style.canopy_color[2] * 0.7 + style.fog_color[2] * 0.3,
                    1.0,
                ];
                let far_mat = renderer.create_material(far_color);
                let far_buf = renderer.create_instance_buffer(&far_strip);
                self.instanced_draws.push(InstancedDrawCall {
                    mesh: billboard_gpu,
                    instance_buffer: far_buf,
                    instance_count: far_strip.len() as u32,
                    material_bind_group: far_mat,
                });
            }
            log::info!(
                "Treeline: {} close, {} far strips",
                close_strip.len(), far_strip.len(),
            );
        }

        // --- Water features ---
        let water_features = detect_water_features(&self.route_points);
        let water_mat = renderer.create_material([0.05, 0.12, 0.18, 0.85]);

        self.water_draws = water_features
            .iter()
            .map(|wf| {
                let mesh = generate_water_mesh(wf, &self.route_points);
                let gpu = GpuMesh::from_cpu(&renderer.device, &mesh);
                DrawCall {
                    mesh: gpu,
                    material_bind_group: water_mat.clone(),
                }
            })
            .collect();

        log::info!("Water: {} features", water_features.len());

        // --- Wildlife ---
        let wildlife = place_wildlife(&self.route_points, &water_features, terrain_config.dem.as_ref());

        // Ground animal meshes
        let squirrel_mesh = GpuMesh::from_cpu(&renderer.device, &generate_squirrel());
        let deer_mesh = GpuMesh::from_cpu(&renderer.device, &generate_deer());
        let turtle_mesh = GpuMesh::from_cpu(&renderer.device, &generate_turtle());
        let egret_mesh = GpuMesh::from_cpu(&renderer.device, &generate_egret());

        // Ground animal materials
        let squirrel_mat = renderer.create_material([0.40, 0.28, 0.15, 1.0]); // brown
        let deer_mat = renderer.create_material([0.45, 0.35, 0.22, 1.0]); // tan
        let turtle_mat = renderer.create_material([0.25, 0.30, 0.15, 1.0]); // dark green
        let egret_mat = renderer.create_material([0.92, 0.90, 0.85, 1.0]); // white

        // Ground animal instance buffers and draw calls
        let squirrel_count = wildlife.squirrels.len() as u32;
        let deer_count = wildlife.deer.len() as u32;
        let turtle_count = wildlife.turtles.len() as u32;
        let egret_count = wildlife.egrets.len() as u32;

        if squirrel_count > 0 {
            let buf = renderer.create_instance_buffer(&wildlife.squirrels);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: squirrel_mesh,
                instance_buffer: buf,
                instance_count: squirrel_count,
                material_bind_group: squirrel_mat,
            });
        }
        if deer_count > 0 {
            let buf = renderer.create_instance_buffer(&wildlife.deer);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: deer_mesh,
                instance_buffer: buf,
                instance_count: deer_count,
                material_bind_group: deer_mat,
            });
        }
        if turtle_count > 0 {
            let buf = renderer.create_instance_buffer(&wildlife.turtles);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: turtle_mesh,
                instance_buffer: buf,
                instance_count: turtle_count,
                material_bind_group: turtle_mat,
            });
        }
        if egret_count > 0 {
            let buf = renderer.create_instance_buffer(&wildlife.egrets);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: egret_mesh,
                instance_buffer: buf,
                instance_count: egret_count,
                material_bind_group: egret_mat,
            });
        }

        log::info!(
            "Ground animals: {} squirrels, {} deer, {} turtles, {} egrets",
            squirrel_count, deer_count, turtle_count, egret_count
        );

        // Flying animal meshes (parameterized birds)
        let crow_cpu = generate_bird(0.06, 0.25, 0.08);
        let hawk_cpu = generate_bird(0.10, 0.50, 0.12);
        let starling_cpu = generate_small_bird();
        let goose_cpu = generate_goose();

        let crow_gpu = GpuMesh::from_cpu(&renderer.device, &crow_cpu);
        let hawk_gpu = GpuMesh::from_cpu(&renderer.device, &hawk_cpu);
        let starling_gpu = GpuMesh::from_cpu(&renderer.device, &starling_cpu);
        let goose_gpu = GpuMesh::from_cpu(&renderer.device, &goose_cpu);

        // Flying animal materials
        let crow_mat = renderer.create_material([0.08, 0.08, 0.10, 1.0]); // near-black
        let hawk_mat = renderer.create_material([0.35, 0.25, 0.15, 1.0]); // brown
        let starling_mat = renderer.create_material([0.12, 0.12, 0.15, 1.0]); // dark grey
        let goose_mat = renderer.create_material([0.40, 0.38, 0.32, 1.0]); // grey-brown

        // Build animated draw calls + CPU copies for each flying group
        let flying_groups: Vec<(Vec<AnimatedInstanceData>, GpuMesh, wgpu::BindGroup)> = vec![
            (wildlife.crows, crow_gpu, crow_mat),
            (wildlife.hawks, hawk_gpu, hawk_mat),
            (wildlife.starlings, starling_gpu, starling_mat),
            (wildlife.geese, goose_gpu, goose_mat),
        ];

        self.animated_draws.clear();
        self.flying_cpu.clear();

        let mut total_flying = 0u32;
        for (cpu_data, mesh, mat) in flying_groups {
            let count = cpu_data.len() as u32;
            total_flying += count;
            let buf = renderer.create_animated_instance_buffer(&cpu_data);
            self.animated_draws.push(AnimatedDrawCall {
                mesh,
                instance_buffer: buf,
                instance_count: count,
                material_bind_group: mat,
            });
            self.flying_cpu.push(cpu_data);
        }

        log::info!(
            "Flying animals: {} crows, {} hawks, {} starlings, {} geese = {} total",
            self.flying_cpu[0].len(),
            self.flying_cpu[1].len(),
            self.flying_cpu[2].len(),
            self.flying_cpu[3].len(),
            total_flying,
        );

        // --- Cabins + Snow banks ---
        self.emissive_instanced_draws.clear();
        if style.cabin_spacing_m > 0.0 {
            let cabin_body_mesh = structures::generate_cabin_body();
            let cabin_window_mesh = structures::generate_cabin_windows();
            let cabin_instances = structures::place_cabins(
                &self.route_points,
                style.vegetation.treeline,
                style.cabin_spacing_m,
            );
            if !cabin_instances.is_empty() {
                let body_gpu = GpuMesh::from_cpu(&renderer.device, &cabin_body_mesh);
                let window_gpu = GpuMesh::from_cpu(&renderer.device, &cabin_window_mesh);
                let body_mat = renderer.create_material([0.42, 0.26, 0.15, 1.0]); // #6B4226 timber
                let window_mat = renderer.create_material([2.0, 1.44, 0.60, 1.0]); // #FFB84D amber glow
                let body_buf = renderer.create_instance_buffer(&cabin_instances);
                let window_buf = renderer.create_instance_buffer(&cabin_instances);
                let count = cabin_instances.len() as u32;

                self.instanced_draws.push(InstancedDrawCall {
                    mesh: body_gpu,
                    instance_buffer: body_buf,
                    instance_count: count,
                    material_bind_group: body_mat,
                });
                self.emissive_instanced_draws.push(InstancedDrawCall {
                    mesh: window_gpu,
                    instance_buffer: window_buf,
                    instance_count: count,
                    material_bind_group: window_mat,
                });
                log::info!("Cabins: {} placed", count);
            }
        }

        // Snow banks (high_alpine_winter only — check if elevation_zones[0] suggests snow)
        if style.elevation_zones[0] < 90000.0 {
            let snow_start = style.elevation_zones[0];
            let snow_instances = structures::place_snow_banks(
                &self.route_points,
                RoadConfig::default().half_width,
                snow_start,
            );
            if !snow_instances.is_empty() {
                let snow_mesh = structures::generate_snow_bank();
                let snow_gpu = GpuMesh::from_cpu(&renderer.device, &snow_mesh);
                let snow_mat = renderer.create_material([0.88, 0.90, 0.95, 1.0]);
                let snow_buf = renderer.create_instance_buffer(&snow_instances);
                let count = snow_instances.len() as u32;

                self.instanced_draws.push(InstancedDrawCall {
                    mesh: snow_gpu,
                    instance_buffer: snow_buf,
                    instance_count: count,
                    material_bind_group: snow_mat,
                });
                log::info!("Snow banks: {} placed", count);
            }
        }

        // --- Cyclists (multi-part colored model) ---
        self.npc_cyclists = spawn_npc_cyclists(self.route_length, 20);
        let cyclist_parts = load_cyclist_glb(include_bytes!("../../../assets/cyclist.glb"));

        self.cyclist_cpu = update_cyclists(
            &mut self.npc_cyclists,
            &self.route_points,
            self.route_length,
            0.0,
        );
        // Reserve slot for the player cyclist (scale 0 = invisible placeholder)
        self.cyclist_cpu.push(CyclistInstanceData {
            position: [0.0, 0.0, 0.0],
            scale: 0.0,
            forward: [0.0, 1.0],
            pedal_phase: 0.0,
            _pad: 0.0,
        });
        let cyclist_buf = renderer.create_cyclist_instance_buffer(&self.cyclist_cpu);
        let cyclist_count = self.cyclist_cpu.len() as u32;

        // Tour de France maillot jaune cyclist colors
        let frame_yellow = [0.95, 0.72, 0.0]; // matching jersey
        let ultegra_silver = [0.55, 0.55, 0.58]; // Shimano Ultegra dark silver
        let ultegra_dark = [0.22, 0.22, 0.24]; // darker metallic accent

        // Generate jersey texture (maillot jaune with bib number "1")
        let (jersey_rgba, jw, jh) = jersey::generate_jersey_texture(
            [255, 191, 0], // maillot jaune yellow
            1,             // bib number
            [0, 0, 0],     // black text
        );
        let (jersey_view, jersey_sampler) = renderer.create_gpu_texture(&jersey_rgba, jw, jh);

        let parts: Vec<CyclistMeshPart> = cyclist_parts
            .into_iter()
            .enumerate()
            .map(|(i, part)| {
                if i == 2 {
                    // Jersey — textured with bib number
                    let mat = renderer.create_textured_cyclist_material(
                        [1.0, 0.75, 0.0, 1.0],
                        &jersey_view,
                        &jersey_sampler,
                    );
                    return CyclistMeshPart {
                        mesh: GpuMesh::from_cpu(&renderer.device, &part.mesh),
                        material_bind_group: mat,
                        pipeline: CyclistPipeline::Textured,
                    };
                }
                if i == 3 {
                    // Skin — realistic skin shader (base_color.w = effort, mid_color.w = sweat_time)
                    let mat = renderer.create_cyclist_material([0.87, 0.67, 0.50, 0.0], None, None);
                    return CyclistMeshPart {
                        mesh: GpuMesh::from_cpu(&renderer.device, &part.mesh),
                        material_bind_group: mat,
                        pipeline: CyclistPipeline::Skin,
                    };
                }
                let mat = match i {
                    0 => renderer.create_cyclist_material([0.02, 0.02, 0.02, 1.0], None, None), // shorts — black
                    1 => renderer.create_cyclist_material([0.12, 0.12, 0.14, 1.0], None, None), // shoes — dark
                    4 => renderer.create_cyclist_material([0.95, 0.95, 0.95, 1.0], None, None), // helmet — white
                    5 => renderer.create_cyclist_material([0.03, 0.03, 0.03, 1.0], None, None), // helmet vents — black
                    6 => renderer.create_cyclist_material([0.04, 0.04, 0.05, 1.0], None, None), // tires — black
                    7 => renderer.create_cyclist_material(
                        part.default_color, Some(frame_yellow), Some(ultegra_silver),
                    ), // frame + components
                    8 => renderer.create_cyclist_material(
                        part.default_color, Some(frame_yellow), Some(ultegra_dark),
                    ), // frame + components metallic
                    _ => renderer.create_cyclist_material(part.default_color, None, None),
                };
                CyclistMeshPart {
                    mesh: GpuMesh::from_cpu(&renderer.device, &part.mesh),
                    material_bind_group: mat,
                    pipeline: CyclistPipeline::Solid,
                }
            })
            .collect();

        self.cyclist_models = vec![CyclistModel {
            parts,
            instance_buffer: cyclist_buf,
            instance_count: cyclist_count,
        }];

        log::info!("Cyclists: {} NPCs", cyclist_count);
    }

    fn route_name(&self) -> &'static str {
        let cat = routes::catalog();
        cat.get(self.current_route).map(|r| r.name).unwrap_or("Unknown")
    }

    fn enter_ride(&mut self) {
        self.load_route(self.current_route);
        self.build_world();
        self.start_time = Instant::now();
        self.prev_t = 0.0;
        self.camera_mode = CameraMode::ThirdPersonClose;
        self.state = AppState::Riding;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Sykla Engine")
            .with_inner_size(winit::dpi::PhysicalSize::new(1440, 810));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let renderer = pollster::block_on(Renderer::new(window.clone()));

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.start_time = Instant::now();

        // Auto-enter first route for quick testing
        self.enter_ride();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                    self.camera.aspect = size.width as f32 / size.height.max(1) as f32;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match self.state {
                        AppState::Selecting => {
                            let route_count = routes::catalog().len();
                            match event.physical_key {
                                PhysicalKey::Code(KeyCode::ArrowUp) => {
                                    self.current_route = if self.current_route == 0 {
                                        route_count - 1
                                    } else {
                                        self.current_route - 1
                                    };
                                }
                                PhysicalKey::Code(KeyCode::ArrowDown) => {
                                    self.current_route = (self.current_route + 1) % route_count;
                                }
                                PhysicalKey::Code(KeyCode::Enter) => {
                                    self.enter_ride();
                                }
                                _ => {}
                            }
                        }
                        AppState::Riding => {
                            match event.physical_key {
                                PhysicalKey::Code(KeyCode::Escape) => {
                                    self.state = AppState::Selecting;
                                }
                                PhysicalKey::Code(KeyCode::KeyC) => {
                                    self.camera_mode = self.camera_mode.next();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                if self.state == AppState::Riding {
                    if let Some(renderer) = &self.renderer {
                        let rects = camera_button_rects(renderer.width as f32, renderer.height as f32);
                        let (cx, cy) = self.cursor_pos;
                        let modes = [CameraMode::ThirdPersonClose, CameraMode::ThirdPersonFar, CameraMode::FirstPerson];
                        for (i, (x, y, w, h)) in rects.iter().enumerate() {
                            if cx >= *x && cx <= x + w && cy >= *y && cy <= y + h {
                                self.camera_mode = modes[i];
                                break;
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                match self.state {
                    AppState::Selecting => self.render_selector(),
                    AppState::Riding => self.render_ride(),
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    fn render_selector(&mut self) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let cat = routes::catalog();
        let route_data: Vec<(&str, &str)> = cat.iter().map(|r| (r.name, r.description)).collect();

        let (hud_verts, hud_indices) = build_selector_hud(
            &route_data,
            self.current_route,
            renderer.width as f32,
            renderer.height as f32,
        );

        // Set camera to a neutral position
        self.camera.eye = Vec3::new(0.0, 100.0, 0.0);
        self.camera.target = Vec3::new(0.0, 100.0, 50.0);
        renderer.update_camera(&self.camera, 0.0);

        let renderer = self.renderer.as_mut().unwrap();
        renderer.update_hud(&hud_verts, &hud_indices);

        match renderer.render(&[], &[], &[], &[], &[], &[], &[], &[], self.sky_color, &[], &[]) {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let (w, h) = (renderer.width, renderer.height);
                renderer.resize(w, h);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {}
            Err(e) => log::error!("Render error: {e:?}"),
        }
    }

    fn render_ride(&mut self) {
        let t = self.start_time.elapsed().as_secs_f32();
        let dt = (t - self.prev_t).min(0.1);
        self.prev_t = t;

        // Physics-based speed
        update_physics(
            &mut self.physics_state,
            &self.physics_params,
            &self.route_points,
            self.route_length,
            dt,
        );

        // Camera follows curved route using physics distance
        let dist = self.physics_state.distance;
        let (px, pz, elev_here, fx, fz) = interpolate_point(&self.route_points, dist as f64);

        // Perpendicular direction for camera offset
        let perp_x = -fz;
        let perp_z = fx;

        // Mode-dependent camera parameters
        let (behind, up, lateral, look_ahead, fov) = self.camera_mode.camera_params();

        self.camera.fov_y = fov.to_radians();

        self.camera.eye = Vec3::new(
            px + perp_x * lateral - fx * behind,
            elev_here + up,
            pz + perp_z * lateral - fz * behind,
        );

        let (ax, az, elev_ahead, _, _) = interpolate_point(&self.route_points, (dist + look_ahead) as f64);
        let target_elev = elev_here.max(elev_ahead) + 2.0;
        self.camera.target = Vec3::new(ax, target_elev, az);

        // Immutable renderer reference for camera/flying updates
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        renderer.update_camera(&self.camera, t);

        // Build HUD geometry
        let route_name = self.route_name();
        let (hud_verts, hud_indices) = build_hud(
            &self.physics_state,
            self.route_length,
            route_name,
            self.camera_mode,
            renderer.width as f32,
            renderer.height as f32,
        );

        // Update grass blade instances when camera moves along route
        if let Some(ref blade_config) = self.grass_blade_config {
            if (dist - self.grass_blade_last_dist).abs() > 3.0 && !self.grass_blade_draws.is_empty() {
                let blades = grass_blades::generate_blade_instances(
                    &self.route_points,
                    blade_config,
                    dist as f64,
                    self.dem_for_grass.as_ref(),
                    self.terrain_half_width,
                    self.terrain_falloff,
                );
                self.grass_blade_draws[0].instance_count = blades.len() as u32;
                if !blades.is_empty() {
                    renderer.queue.write_buffer(
                        &self.grass_blade_draws[0].instance_buffer,
                        0,
                        bytemuck::cast_slice(&blades),
                    );
                }
                self.grass_blade_last_dist = dist;
            }
        }

        // Update flying animal positions (CPU → GPU)
        // Flying animals move freely in world space
        for (cpu_data, draw) in self.flying_cpu.iter_mut().zip(self.animated_draws.iter()) {
            if cpu_data.is_empty() { continue; }
            for inst in cpu_data.iter_mut() {
                inst.position[0] += inst.velocity[0] * dt;
                inst.position[1] += inst.velocity[1] * dt;
                inst.position[2] += inst.velocity[2] * dt;
            }
            renderer.queue.write_buffer(
                &draw.instance_buffer,
                0,
                bytemuck::cast_slice(cpu_data),
            );
        }

        // Update cyclist positions (CPU → GPU)
        self.cyclist_cpu = update_cyclists(
            &mut self.npc_cyclists,
            &self.route_points,
            self.route_length,
            dt,
        );
        // Add player cyclist (visible in 3rd-person modes)
        if self.camera_mode != CameraMode::FirstPerson {
            let player_pedal_phase = (dist / 5.3) * std::f32::consts::TAU;
            self.cyclist_cpu.push(CyclistInstanceData {
                position: [px, elev_here + 0.08, pz],
                scale: 1.0,
                forward: [fx, fz],
                pedal_phase: player_pedal_phase,
                _pad: 0.0,
            });
        }
        if !self.cyclist_models.is_empty() {
            self.cyclist_models[0].instance_count = self.cyclist_cpu.len() as u32;
            renderer.queue.write_buffer(
                &self.cyclist_models[0].instance_buffer,
                0,
                bytemuck::cast_slice(&self.cyclist_cpu),
            );
        }

        // Update skeleton bone matrices (CPU skinning → GPU storage buffer)
        self.skeleton_system.update(&self.cyclist_cpu);
        renderer.update_bone_matrices(&self.skeleton_system.gpu_matrices);

        // Mutable renderer reference for HUD update + render
        let renderer = self.renderer.as_mut().unwrap();
        renderer.update_hud(&hud_verts, &hud_indices);

        match renderer.render(
            &self.draws,
            &self.road_draws,
            &self.grass_shell_draws,
            &self.grass_blade_draws,
            &self.instanced_draws,
            &self.animated_draws,
            &self.cyclist_models,
            &self.water_draws,
            self.sky_color,
            &self.emissive_instanced_draws,
            &self.tree_models,
        ) {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let (w, h) = (renderer.width, renderer.height);
                renderer.resize(w, h);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {}
            Err(e) => log::error!("Render error: {e:?}"),
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
