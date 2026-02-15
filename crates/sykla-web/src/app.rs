use std::sync::Arc;

use sykla_engine::camera::{Camera, CameraMode};
use sykla_engine::cyclist::{
    CyclistInstanceData, NpcCyclist, load_cyclist_glb, spawn_npc_cyclists, update_cyclists,
};
use sykla_engine::dem::{DemTile, GeoOrigin};
use sykla_engine::skeleton::SkeletonSystem;
use sykla_engine::glam::Vec3;
use sykla_engine::hud::{build_hud, build_selector_hud, camera_button_rects};
use sykla_engine::mesh::{CpuMesh, GpuMesh, Vertex};
use sykla_engine::physics::{PhysicsParams, PhysicsState, update_physics};
use sykla_engine::mountains::{generate_mountain_ring, generate_mountain_ring_with_radius};
use sykla_engine::grass;
use sykla_engine::grass_blades;
use sykla_engine::jersey;
use sykla_engine::renderer::{AnimatedDrawCall, CyclistMeshPart, CyclistModel, CyclistPipeline, DrawCall, GrassBladeDrawCall, GrassShellDraw, InstancedDrawCall, Renderer, TreeModel};
use sykla_engine::road::{generate_roads_by_surface, RoadConfig};
use sykla_engine::routes::{self, RoadMarkings};
use sykla_engine::structures;
use sykla_engine::terrain::{
    generate_ground_plane, generate_terrain, interpolate_point, DemTerrainSource, RoutePoint, SurfaceType,
};
use sykla_engine::vegetation::{
    generate_bush, generate_canopy, generate_pine_canopy, generate_pine_trunk, generate_trunk,
    place_vegetation, InstanceData,
};
use sykla_engine::treeline::{generate_billboard_quad, generate_treeline_strips, TreelineConfig};
use sykla_engine::water::{detect_water_features, generate_water_mesh};
use sykla_engine::wildlife::{
    generate_bird, generate_deer, generate_egret, generate_goose, generate_small_bird,
    generate_squirrel, generate_turtle, place_wildlife, AnimatedInstanceData,
};
use winit::event::{ElementState, KeyEvent, TouchPhase};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

use sykla_core::ride_engine::RideEngine;
use sykla_world::city::{CityData, CityMesh};
use sykla_world::road_network::RoadNetwork;

use crate::ble::TrainerState;
use crate::city_loader::CityLoadState;
use crate::navigation::{KeyState, NavigationState};

#[derive(PartialEq)]
enum AppState {
    Selecting,
    Riding,
    CityRoam,
}

pub struct AppInner {
    renderer: Renderer,
    #[allow(dead_code)]
    window: Arc<Window>,
    state: AppState,
    camera: Camera,
    draws: Vec<DrawCall>,
    road_draws: Vec<DrawCall>,
    instanced_draws: Vec<InstancedDrawCall>,
    animated_draws: Vec<AnimatedDrawCall>,
    water_draws: Vec<DrawCall>,
    flying_cpu: Vec<Vec<AnimatedInstanceData>>,
    prev_t: f32,
    route_length: f32,
    route_points: Vec<RoutePoint>,
    geo_origin: Option<GeoOrigin>,
    physics_state: PhysicsState,
    physics_params: PhysicsParams,
    current_route: usize,
    current_key: &'static str,
    // City mode
    city_load: CityLoadState,
    road_network: Option<RoadNetwork>,
    nav: NavigationState,
    key_state: KeyState,
    // BLE trainer
    trainer: TrainerState,
    // Ride engine (for route riding with trainer)
    ride_engine: Option<RideEngine>,
    sky_color: [f32; 3],
    // Camera mode
    camera_mode: CameraMode,
    cursor_pos: (f32, f32),
    // Cyclist rendering
    cyclist_models: Vec<CyclistModel>,
    cyclist_cpu: Vec<CyclistInstanceData>,
    npc_cyclists: Vec<NpcCyclist>,
    skeleton_system: SkeletonSystem,
    emissive_instanced_draws: Vec<InstancedDrawCall>,
    tree_models: Vec<TreeModel>,
    grass_shell_draws: Vec<GrassShellDraw>,
    grass_blade_draws: Vec<GrassBladeDrawCall>,
    grass_blade_config: Option<grass_blades::GrassBladeConfig>,
    grass_blade_last_dist: f32,
    dem_for_grass: Option<DemTerrainSource>,
    terrain_half_width: f32,
    terrain_falloff: f32,
}

impl AppInner {
    pub fn new(renderer: Renderer, window: Arc<Window>) -> Self {
        let aspect = renderer.width as f32 / renderer.height.max(1) as f32;
        let mut app = Self {
            renderer,
            window,
            state: AppState::Selecting,
            camera: Camera::new(aspect),
            draws: Vec::new(),
            road_draws: Vec::new(),
            instanced_draws: Vec::new(),
            animated_draws: Vec::new(),
            water_draws: Vec::new(),
            flying_cpu: Vec::new(),
            prev_t: 0.0,
            route_length: 0.0,
            route_points: Vec::new(),
            geo_origin: None,
            physics_state: PhysicsState::default(),
            physics_params: PhysicsParams::default(),
            current_route: 0,
            current_key: "att",
            city_load: CityLoadState::new(),
            road_network: None,
            nav: NavigationState::default(),
            key_state: KeyState::default(),
            trainer: TrainerState::default(),
            ride_engine: None,
            sky_color: [0.52, 0.70, 0.82],
            camera_mode: CameraMode::ThirdPersonClose,
            cursor_pos: (0.0, 0.0),
            cyclist_models: Vec::new(),
            cyclist_cpu: Vec::new(),
            npc_cyclists: Vec::new(),
            skeleton_system: SkeletonSystem::new(32),
            emissive_instanced_draws: Vec::new(),
            tree_models: Vec::new(),
            grass_shell_draws: Vec::new(),
            grass_blade_draws: Vec::new(),
            grass_blade_config: None,
            grass_blade_last_dist: -999.0,
            dem_for_grass: None,
            terrain_half_width: 300.0,
            terrain_falloff: 0.08,
        };

        // Start city data loading
        app.city_load.trigger_load();

        // Auto-enter ride for quick testing
        app.enter_ride();

        app
    }

    pub fn handle_resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.camera.aspect = width as f32 / height.max(1) as f32;
    }

    pub fn handle_key(&mut self, event: &KeyEvent) {
        // Update persistent key state
        let code = match event.physical_key {
            PhysicalKey::Code(c) => c,
            _ => return,
        };
        let pressed = event.state == ElementState::Pressed;
        self.key_state.update(code, pressed);

        if event.state != ElementState::Pressed {
            return;
        }

        match self.state {
            AppState::Selecting => {
                let route_count = routes::catalog().len();
                match code {
                    KeyCode::ArrowUp => {
                        self.current_route = if self.current_route == 0 {
                            route_count - 1
                        } else {
                            self.current_route - 1
                        };
                    }
                    KeyCode::ArrowDown => {
                        self.current_route = (self.current_route + 1) % route_count;
                    }
                    KeyCode::Enter => {
                        self.enter_ride();
                    }
                    KeyCode::Tab => {
                        // Switch to city roam if city is loaded
                        if self.city_load.loaded {
                            self.state = AppState::CityRoam;
                        }
                    }
                    _ => {}
                }
            }
            AppState::Riding => match code {
                KeyCode::Escape => {
                    self.state = AppState::Selecting;
                    self.ride_engine = None;
                }
                KeyCode::KeyC => {
                    self.camera_mode = self.camera_mode.next();
                }
                KeyCode::Tab => {
                    if self.city_load.loaded {
                        self.state = AppState::CityRoam;
                        self.ride_engine = None;
                    }
                }
                _ => {}
            },
            AppState::CityRoam => match code {
                KeyCode::Escape => {
                    self.state = AppState::Selecting;
                }
                _ => {
                    // Navigation handles its own key events via key_state
                }
            },
        }
    }

    pub fn render_frame(&mut self, t: f32) {
        let dt = (t - self.prev_t).min(0.1);
        self.prev_t = t;

        // Poll trainer data each frame
        self.trainer.poll();

        // Check city load state — extract data first to avoid borrow conflict
        self.check_city_load();

        match self.state {
            AppState::Selecting => self.render_selector(),
            AppState::Riding => self.render_ride(t, dt),
            AppState::CityRoam => self.render_city_roam(dt),
        }
    }

    fn check_city_load(&mut self) {
        if self.city_load.loaded || !self.city_load.loading {
            return;
        }

        let city_data = crate::city_loader::take_loaded_city();
        let Some(result) = city_data else {
            return;
        };

        match result {
            Ok(city_data) => {
                web_sys::console::log_1(
                    &format!(
                        "City loaded: {} ({} buildings, {} landuse, {} water)",
                        city_data.name,
                        city_data.buildings.len(),
                        city_data.landuse.len(),
                        city_data.water.len(),
                    )
                    .into(),
                );
                self.build_city_world(&city_data);
                self.city_load.loaded = true;
                self.city_load.loading = false;
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to load city: {e}").into());
                self.city_load.error = Some(e);
                self.city_load.loading = false;
            }
        }
    }

    fn load_route(&mut self, index: usize) {
        let cat = routes::catalog();
        let key = cat[index].key;
        let (points, geo_origin) = routes::generate_route(key);
        self.route_points = points;
        self.geo_origin = geo_origin;
        self.route_length = self
            .route_points
            .last()
            .map(|p| p.distance_m as f32)
            .unwrap_or(0.0);
        self.current_route = index;
        self.current_key = key;
        self.physics_state = PhysicsState::default();
    }

    fn build_world(&mut self) {
        let style = routes::route_style(self.current_key);

        // Update shadow VP light direction
        self.renderer.light_dir = Vec3::new(
            style.light_direction[0],
            style.light_direction[1],
            style.light_direction[2],
        ).normalize();

        // Terrain, ground plane, and road
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
        let road_segments =
            generate_roads_by_surface(&self.route_points, &RoadConfig::default());

        let terrain_gpu = GpuMesh::from_cpu(&self.renderer.device, &terrain_mesh);
        let ground_gpu = GpuMesh::from_cpu(&self.renderer.device, &ground_mesh);

        let terrain_mat = self.renderer.create_terrain_material(
            style.terrain_color,
            style.terrain_mid_color,
            style.terrain_high_color,
            style.elevation_zones,
            style.terrain_params,
        );
        let ground_mat = self.renderer.create_material(style.ground_color);

        // Set lighting, fog, and sky from route style
        self.renderer.set_light_direction_uniform(style.light_direction);
        self.renderer.set_light_color(style.light_color);
        self.renderer.set_light_ambient(style.light_ambient);
        self.renderer.set_fog_color(style.fog_color);
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
                let mountain_gpu = GpuMesh::from_cpu(&self.renderer.device, &mountain_mesh);
                let mountain_mat = self.renderer.create_terrain_material(
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

        // Road segments
        let has_markings = style.road_markings == RoadMarkings::European;
        self.road_draws = Vec::new();
        for (mesh, surface) in &road_segments {
            let color = match surface {
                SurfaceType::Paved => style.road_color,
                SurfaceType::Gravel => style.gravel_color,
            };
            let gpu = GpuMesh::from_cpu(&self.renderer.device, mesh);
            let mat = self.renderer.create_road_material(color, has_markings);
            self.road_draws.push(DrawCall {
                mesh: gpu,
                material_bind_group: mat,
            });
        }

        // Grass shells (volumetric grass on terrain)
        let mut grass_shell_draws = Vec::new();
        if let Some(grass_params) = grass::GrassParams::for_biome(style.terrain_params[3]) {
            let grass_mesh = GpuMesh::from_cpu(&self.renderer.device, &terrain_mesh);
            let grass_mat = self.renderer.create_grass_material(&grass_params);
            grass_shell_draws.push(GrassShellDraw {
                mesh: grass_mesh,
                material_bind_group: grass_mat,
                num_shells: grass_params.num_shells,
            });
        }
        self.grass_shell_draws = grass_shell_draws;

        // Grass blades (instanced individual blades near camera)
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
            let blade_gpu = GpuMesh::from_cpu(&self.renderer.device, &blade_mesh);
            let blade_mat = self.renderer.create_grass_blade_material(blade_config);
            let blade_buf = self.renderer.create_grass_blade_instance_buffer(grass_blades::MAX_GRASS_BLADES);

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
                self.renderer.queue.write_buffer(
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
        }

        // Vegetation (instanced)
        let placement = place_vegetation(
            &self.route_points, &style.vegetation, terrain_config.dem.as_ref(),
            terrain_config.half_width, terrain_config.falloff,
        );

        let trunk_mesh = generate_trunk();
        let canopy_mesh = generate_canopy();
        let pine_trunk_mesh = generate_pine_trunk();
        let pine_canopy_mesh = generate_pine_canopy();
        let bush_mesh_cpu = generate_bush();

        let dec_trunk_gpu = GpuMesh::from_cpu(&self.renderer.device, &trunk_mesh);
        let dec_canopy_gpu = GpuMesh::from_cpu(&self.renderer.device, &canopy_mesh);
        let pine_trunk_gpu = GpuMesh::from_cpu(&self.renderer.device, &pine_trunk_mesh);
        let pine_canopy_gpu = GpuMesh::from_cpu(&self.renderer.device, &pine_canopy_mesh);
        let bush_gpu = GpuMesh::from_cpu(&self.renderer.device, &bush_mesh_cpu);

        let ydec_trunk_gpu = GpuMesh::from_cpu(&self.renderer.device, &trunk_mesh);
        let ydec_canopy_gpu = GpuMesh::from_cpu(&self.renderer.device, &canopy_mesh);
        let ypine_trunk_gpu = GpuMesh::from_cpu(&self.renderer.device, &pine_trunk_mesh);
        let ypine_canopy_gpu = GpuMesh::from_cpu(&self.renderer.device, &pine_canopy_mesh);

        let dec_buf1 = self.renderer.create_instance_buffer(&placement.deciduous);
        let dec_buf2 = self.renderer.create_instance_buffer(&placement.deciduous);
        let pine_buf1 = self.renderer.create_instance_buffer(&placement.pine);
        let pine_buf2 = self.renderer.create_instance_buffer(&placement.pine);
        let bush_buf = self.renderer.create_instance_buffer(&placement.bush);
        let ydec_buf1 = self
            .renderer
            .create_instance_buffer(&placement.young_deciduous);
        let ydec_buf2 = self
            .renderer
            .create_instance_buffer(&placement.young_deciduous);
        let ypine_buf1 = self
            .renderer
            .create_instance_buffer(&placement.young_pine);
        let ypine_buf2 = self
            .renderer
            .create_instance_buffer(&placement.young_pine);

        let dec_count = placement.deciduous.len() as u32;
        let pine_count = placement.pine.len() as u32;
        let bush_count = placement.bush.len() as u32;
        let ydec_count = placement.young_deciduous.len() as u32;
        let ypine_count = placement.young_pine.len() as u32;

        let trunk_mat = self.renderer.create_material(style.trunk_color);
        let dec_canopy_mat = self.renderer.create_material(style.canopy_color);
        let pine_trunk_mat = self.renderer.create_material(style.trunk_color);
        // Winter routes: snow on pine canopies (detected by elevation_zones[0] < 90000)
        let is_winter = style.elevation_zones[0] < 90000.0;
        let pine_canopy_mat = if is_winter {
            self.renderer.create_snow_foliage_material(style.pine_color)
        } else {
            self.renderer.create_material(style.pine_color)
        };
        let bush_mat = self.renderer.create_material(style.bush_color);
        let young_trunk_mat = self.renderer.create_material([
            style.trunk_color[0] + 0.04,
            style.trunk_color[1] + 0.04,
            style.trunk_color[2] + 0.02,
            1.0,
        ]);
        let young_dec_canopy_mat = self.renderer.create_material([
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
            self.renderer.create_snow_foliage_material(young_pine_color)
        } else {
            self.renderer.create_material(young_pine_color)
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

        // Textured GLB trees (eastern species)
        self.tree_models.clear();
        if style.vegetation.use_eastern_species {
            let oak_parts = sykla_engine::trees::load_tree_species(&self.renderer, include_bytes!("../../../assets/trees/oak.glb"));
            let pine_parts = sykla_engine::trees::load_tree_species(&self.renderer, include_bytes!("../../../assets/trees/loblolly_pine.glb"));
            let redbud_parts = sykla_engine::trees::load_tree_species(&self.renderer, include_bytes!("../../../assets/trees/redbud.glb"));

            for (parts, instances) in [
                (oak_parts, &placement.oaks),
                (pine_parts, &placement.loblolly_pines),
                (redbud_parts, &placement.redbuds),
            ] {
                if !instances.is_empty() {
                    let buf = self.renderer.create_tree_instance_buffer(instances);
                    self.tree_models.push(TreeModel {
                        parts,
                        instance_buffer: buf,
                        instance_count: instances.len() as u32,
                    });
                }
            }
            if !placement.fallen_trees.is_empty() {
                let fallen_parts = sykla_engine::trees::load_tree_species(&self.renderer, include_bytes!("../../../assets/trees/oak.glb"));
                let buf = self.renderer.create_tree_instance_buffer(&placement.fallen_trees);
                self.tree_models.push(TreeModel {
                    parts: fallen_parts,
                    instance_buffer: buf,
                    instance_count: placement.fallen_trees.len() as u32,
                });
            }
        }

        // Treeline strips (distant billboard trees)
        // Skip when eastern species GLB trees are active
        if !style.vegetation.use_eastern_species {
            let treeline_config = TreelineConfig::default();
            let (close_strip, far_strip) = generate_treeline_strips(
                &self.route_points,
                &treeline_config,
                None,
            );

            if !close_strip.is_empty() {
                let billboard_mesh = generate_billboard_quad();
                let billboard_gpu = GpuMesh::from_cpu(&self.renderer.device, &billboard_mesh);
                let close_mat = self.renderer.create_material(style.canopy_color);
                let close_buf = self.renderer.create_instance_buffer(&close_strip);
                self.instanced_draws.push(InstancedDrawCall {
                    mesh: billboard_gpu,
                    instance_buffer: close_buf,
                    instance_count: close_strip.len() as u32,
                    material_bind_group: close_mat,
                });
            }
            if !far_strip.is_empty() {
                let billboard_mesh = generate_billboard_quad();
                let billboard_gpu = GpuMesh::from_cpu(&self.renderer.device, &billboard_mesh);
                let far_color = [
                    style.canopy_color[0] * 0.7 + style.fog_color[0] * 0.3,
                    style.canopy_color[1] * 0.7 + style.fog_color[1] * 0.3,
                    style.canopy_color[2] * 0.7 + style.fog_color[2] * 0.3,
                    1.0,
                ];
                let far_mat = self.renderer.create_material(far_color);
                let far_buf = self.renderer.create_instance_buffer(&far_strip);
                self.instanced_draws.push(InstancedDrawCall {
                    mesh: billboard_gpu,
                    instance_buffer: far_buf,
                    instance_count: far_strip.len() as u32,
                    material_bind_group: far_mat,
                });
            }
        }

        // Water features
        let water_features = detect_water_features(&self.route_points);
        let water_mat = self.renderer.create_material([0.05, 0.12, 0.18, 0.85]);

        self.water_draws = water_features
            .iter()
            .map(|wf| {
                let mesh = generate_water_mesh(wf, &self.route_points);
                let gpu = GpuMesh::from_cpu(&self.renderer.device, &mesh);
                DrawCall {
                    mesh: gpu,
                    material_bind_group: water_mat.clone(),
                }
            })
            .collect();

        // Wildlife
        let wildlife = place_wildlife(&self.route_points, &water_features, terrain_config.dem.as_ref());

        // Ground animals
        let squirrel_mesh = GpuMesh::from_cpu(&self.renderer.device, &generate_squirrel());
        let deer_mesh = GpuMesh::from_cpu(&self.renderer.device, &generate_deer());
        let turtle_mesh = GpuMesh::from_cpu(&self.renderer.device, &generate_turtle());
        let egret_mesh = GpuMesh::from_cpu(&self.renderer.device, &generate_egret());

        let squirrel_mat = self.renderer.create_material([0.40, 0.28, 0.15, 1.0]);
        let deer_mat = self.renderer.create_material([0.45, 0.35, 0.22, 1.0]);
        let turtle_mat = self.renderer.create_material([0.25, 0.30, 0.15, 1.0]);
        let egret_mat = self.renderer.create_material([0.92, 0.90, 0.85, 1.0]);

        let squirrel_count = wildlife.squirrels.len() as u32;
        let deer_count = wildlife.deer.len() as u32;
        let turtle_count = wildlife.turtles.len() as u32;
        let egret_count = wildlife.egrets.len() as u32;

        if squirrel_count > 0 {
            let buf = self.renderer.create_instance_buffer(&wildlife.squirrels);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: squirrel_mesh,
                instance_buffer: buf,
                instance_count: squirrel_count,
                material_bind_group: squirrel_mat,
            });
        }
        if deer_count > 0 {
            let buf = self.renderer.create_instance_buffer(&wildlife.deer);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: deer_mesh,
                instance_buffer: buf,
                instance_count: deer_count,
                material_bind_group: deer_mat,
            });
        }
        if turtle_count > 0 {
            let buf = self.renderer.create_instance_buffer(&wildlife.turtles);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: turtle_mesh,
                instance_buffer: buf,
                instance_count: turtle_count,
                material_bind_group: turtle_mat,
            });
        }
        if egret_count > 0 {
            let buf = self.renderer.create_instance_buffer(&wildlife.egrets);
            self.instanced_draws.push(InstancedDrawCall {
                mesh: egret_mesh,
                instance_buffer: buf,
                instance_count: egret_count,
                material_bind_group: egret_mat,
            });
        }

        // Flying animals
        let crow_cpu = generate_bird(0.06, 0.25, 0.08);
        let hawk_cpu = generate_bird(0.10, 0.50, 0.12);
        let starling_cpu = generate_small_bird();
        let goose_cpu = generate_goose();

        let crow_gpu = GpuMesh::from_cpu(&self.renderer.device, &crow_cpu);
        let hawk_gpu = GpuMesh::from_cpu(&self.renderer.device, &hawk_cpu);
        let starling_gpu = GpuMesh::from_cpu(&self.renderer.device, &starling_cpu);
        let goose_gpu = GpuMesh::from_cpu(&self.renderer.device, &goose_cpu);

        let crow_mat = self.renderer.create_material([0.08, 0.08, 0.10, 1.0]);
        let hawk_mat = self.renderer.create_material([0.35, 0.25, 0.15, 1.0]);
        let starling_mat = self.renderer.create_material([0.12, 0.12, 0.15, 1.0]);
        let goose_mat = self.renderer.create_material([0.40, 0.38, 0.32, 1.0]);

        let flying_groups: Vec<(Vec<AnimatedInstanceData>, GpuMesh, wgpu::BindGroup)> = vec![
            (wildlife.crows, crow_gpu, crow_mat),
            (wildlife.hawks, hawk_gpu, hawk_mat),
            (wildlife.starlings, starling_gpu, starling_mat),
            (wildlife.geese, goose_gpu, goose_mat),
        ];

        self.animated_draws.clear();
        self.flying_cpu.clear();

        for (cpu_data, mesh, mat) in flying_groups {
            let count = cpu_data.len() as u32;
            let buf = self.renderer.create_animated_instance_buffer(&cpu_data);
            self.animated_draws.push(AnimatedDrawCall {
                mesh,
                instance_buffer: buf,
                instance_count: count,
                material_bind_group: mat,
            });
            self.flying_cpu.push(cpu_data);
        }

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
                let body_gpu = GpuMesh::from_cpu(&self.renderer.device, &cabin_body_mesh);
                let window_gpu = GpuMesh::from_cpu(&self.renderer.device, &cabin_window_mesh);
                let body_mat = self.renderer.create_material([0.42, 0.26, 0.15, 1.0]); // #6B4226 timber
                let window_mat = self.renderer.create_material([2.0, 1.44, 0.60, 1.0]); // #FFB84D amber glow
                let body_buf = self.renderer.create_instance_buffer(&cabin_instances);
                let window_buf = self.renderer.create_instance_buffer(&cabin_instances);
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
            }
        }

        // Snow banks
        if style.elevation_zones[0] < 90000.0 {
            let snow_start = style.elevation_zones[0];
            let snow_instances = structures::place_snow_banks(
                &self.route_points,
                RoadConfig::default().half_width,
                snow_start,
            );
            if !snow_instances.is_empty() {
                let snow_mesh = structures::generate_snow_bank();
                let snow_gpu = GpuMesh::from_cpu(&self.renderer.device, &snow_mesh);
                let snow_mat = self.renderer.create_material([0.88, 0.90, 0.95, 1.0]);
                let snow_buf = self.renderer.create_instance_buffer(&snow_instances);
                let count = snow_instances.len() as u32;

                self.instanced_draws.push(InstancedDrawCall {
                    mesh: snow_gpu,
                    instance_buffer: snow_buf,
                    instance_count: count,
                    material_bind_group: snow_mat,
                });
            }
        }

        // Cyclists (multi-part colored model)
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
        let cyclist_buf = self.renderer.create_cyclist_instance_buffer(&self.cyclist_cpu);
        let cyclist_count = self.cyclist_cpu.len() as u32;

        // Tour de France maillot jaune cyclist colors
        let frame_yellow = [0.95, 0.72, 0.0];
        let ultegra_silver = [0.55, 0.55, 0.58];
        let ultegra_dark = [0.22, 0.22, 0.24];

        // Generate jersey texture (maillot jaune with bib number "1")
        let (jersey_rgba, jw, jh) = jersey::generate_jersey_texture(
            [255, 191, 0], // maillot jaune yellow
            1,             // bib number
            [0, 0, 0],     // black text
        );
        let (jersey_view, jersey_sampler) = self.renderer.create_gpu_texture(&jersey_rgba, jw, jh);

        let parts: Vec<CyclistMeshPart> = cyclist_parts
            .into_iter()
            .enumerate()
            .map(|(i, part)| {
                if i == 2 {
                    // Jersey — textured with bib number
                    let mat = self.renderer.create_textured_cyclist_material(
                        [1.0, 0.75, 0.0, 1.0],
                        &jersey_view,
                        &jersey_sampler,
                    );
                    return CyclistMeshPart {
                        mesh: GpuMesh::from_cpu(&self.renderer.device, &part.mesh),
                        material_bind_group: mat,
                        pipeline: CyclistPipeline::Textured,
                    };
                }
                if i == 3 {
                    // Skin — realistic skin shader
                    let mat = self.renderer.create_cyclist_material([0.87, 0.67, 0.50, 0.0], None, None);
                    return CyclistMeshPart {
                        mesh: GpuMesh::from_cpu(&self.renderer.device, &part.mesh),
                        material_bind_group: mat,
                        pipeline: CyclistPipeline::Skin,
                    };
                }
                let mat = match i {
                    0 => self.renderer.create_cyclist_material([0.02, 0.02, 0.02, 1.0], None, None),
                    1 => self.renderer.create_cyclist_material([0.12, 0.12, 0.14, 1.0], None, None),
                    4 => self.renderer.create_cyclist_material([0.95, 0.95, 0.95, 1.0], None, None),
                    5 => self.renderer.create_cyclist_material([0.03, 0.03, 0.03, 1.0], None, None),
                    6 => self.renderer.create_cyclist_material([0.04, 0.04, 0.05, 1.0], None, None),
                    7 => self.renderer.create_cyclist_material(
                        part.default_color, Some(frame_yellow), Some(ultegra_silver),
                    ),
                    8 => self.renderer.create_cyclist_material(
                        part.default_color, Some(frame_yellow), Some(ultegra_dark),
                    ),
                    _ => self.renderer.create_cyclist_material(part.default_color, None, None),
                };
                CyclistMeshPart {
                    mesh: GpuMesh::from_cpu(&self.renderer.device, &part.mesh),
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
    }

    fn route_name(&self) -> &'static str {
        let cat = routes::catalog();
        cat.get(self.current_route)
            .map(|r| r.name)
            .unwrap_or("Unknown")
    }

    fn enter_ride(&mut self) {
        self.load_route(self.current_route);
        self.build_world();
        self.prev_t = 0.0;
        self.state = AppState::Riding;
    }

    fn render_selector(&mut self) {
        let cat = routes::catalog();
        let route_data: Vec<(&str, &str)> =
            cat.iter().map(|r| (r.name, r.description)).collect();

        let (hud_verts, hud_indices) = build_selector_hud(
            &route_data,
            self.current_route,
            self.renderer.width as f32,
            self.renderer.height as f32,
        );

        self.camera.eye = Vec3::new(0.0, 100.0, 0.0);
        self.camera.target = Vec3::new(0.0, 100.0, 50.0);
        self.renderer.update_camera(&self.camera, 0.0);
        self.renderer.update_hud(&hud_verts, &hud_indices);

        match self.renderer.render(&[], &[], &[], &[], &[], &[], &[], &[], self.sky_color, &[], &[]) {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let (w, h) = (self.renderer.width, self.renderer.height);
                self.renderer.resize(w, h);
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Render error: {e:?}").into());
            }
        }
    }

    fn render_ride(&mut self, t: f32, dt: f32) {
        // Update ride engine with trainer data if present
        if let Some(ref mut engine) = self.ride_engine {
            engine.update(
                dt as f64,
                self.trainer.speed_kmh,
                self.trainer.power_watts,
                self.trainer.cadence_rpm,
            );
        }

        // Physics-based speed
        update_physics(
            &mut self.physics_state,
            &self.physics_params,
            &self.route_points,
            self.route_length,
            dt,
        );

        // Camera follows curved route (mode-based)
        let dist = self.physics_state.distance;
        let (px, pz, elev_here, fx, fz) =
            interpolate_point(&self.route_points, dist as f64);

        let (behind, up, lateral, look_ahead, fov) = self.camera_mode.camera_params();
        self.camera.fov_y = fov.to_radians();

        let perp_x = -fz;
        let perp_z = fx;

        self.camera.eye = Vec3::new(
            px + perp_x * lateral - fx * behind,
            elev_here + up,
            pz + perp_z * lateral - fz * behind,
        );

        let (ax, az, elev_ahead, _, _) =
            interpolate_point(&self.route_points, (dist + look_ahead) as f64);
        let target_elev = elev_here.max(elev_ahead) + 2.0;
        self.camera.target = Vec3::new(ax, target_elev, az);

        self.renderer.update_camera(&self.camera, t);

        // Build HUD
        let route_name = self.route_name();
        let (hud_verts, hud_indices) = build_hud(
            &self.physics_state,
            self.route_length,
            route_name,
            self.camera_mode,
            self.renderer.width as f32,
            self.renderer.height as f32,
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
                    self.renderer.queue.write_buffer(
                        &self.grass_blade_draws[0].instance_buffer,
                        0,
                        bytemuck::cast_slice(&blades),
                    );
                }
                self.grass_blade_last_dist = dist;
            }
        }

        // Update flying animal positions
        for (cpu_data, draw) in self.flying_cpu.iter_mut().zip(self.animated_draws.iter()) {
            if cpu_data.is_empty() {
                continue;
            }
            for inst in cpu_data.iter_mut() {
                inst.position[0] += inst.velocity[0] * dt;
                inst.position[1] += inst.velocity[1] * dt;
                inst.position[2] += inst.velocity[2] * dt;
            }
            self.renderer.queue.write_buffer(
                &draw.instance_buffer,
                0,
                bytemuck::cast_slice(cpu_data),
            );
        }

        // Update cyclist positions
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
            self.renderer.queue.write_buffer(
                &self.cyclist_models[0].instance_buffer,
                0,
                bytemuck::cast_slice(&self.cyclist_cpu),
            );
        }

        // Update skeleton bone matrices (CPU skinning → GPU storage buffer)
        self.skeleton_system.update(&self.cyclist_cpu);
        self.renderer.update_bone_matrices(&self.skeleton_system.gpu_matrices);

        self.renderer.update_hud(&hud_verts, &hud_indices);

        match self.renderer.render(
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
                let (w, h) = (self.renderer.width, self.renderer.height);
                self.renderer.resize(w, h);
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Render error: {e:?}").into());
            }
        }
    }

    fn render_city_roam(&mut self, dt: f32) {
        // Update navigation
        let (eye, target) = self
            .nav
            .update(dt, &self.key_state, self.road_network.as_ref());

        self.camera.eye = eye;
        self.camera.target = target;
        self.renderer.update_camera(&self.camera, 0.0);

        // Build simple HUD with trainer data + street name + mode
        let street_name = self.nav.current_street_name(self.road_network.as_ref());
        let mode_label = self.nav.mode_label();
        let _hud_text = format!(
            "{:.1} km/h  {:.0} W  {:.0} rpm\n{}\n{}",
            self.trainer.speed_kmh,
            self.trainer.power_watts,
            self.trainer.cadence_rpm,
            street_name.unwrap_or(""),
            mode_label,
        );
        // Use the engine's HUD (empty for now — the renderer still gets the sky-blue clear)
        self.renderer.update_hud(&[], &[]);

        match self.renderer.render(
            &self.draws,
            &self.road_draws,
            &[],
            &[],
            &self.instanced_draws,
            &self.animated_draws,
            &[],
            &self.water_draws,
            self.sky_color,
            &[],
            &[],
        ) {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let (w, h) = (self.renderer.width, self.renderer.height);
                self.renderer.resize(w, h);
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Render error: {e:?}").into());
            }
        }
    }

    fn build_city_world(&mut self, city_data: &CityData) {
        // Store road network for navigation
        self.road_network = city_data.road_network.clone();

        // Clear existing draws (they'll be rebuilt)
        let mut city_draws: Vec<DrawCall> = Vec::new();
        let mut city_road_draws: Vec<DrawCall> = Vec::new();

        // Convert all CityMesh to DrawCalls
        let all_meshes = city_data
            .buildings
            .iter()
            .chain(city_data.landuse.iter())
            .chain(city_data.water.iter())
            .chain(city_data.roads.iter());

        for city_mesh in all_meshes {
            if city_mesh.vertices.is_empty() {
                continue;
            }
            let draw = city_mesh_to_draw(&self.renderer, city_mesh);

            use sykla_world::city::FeatureSubtype;
            let is_road = matches!(
                city_mesh.feature_subtype,
                FeatureSubtype::Highway
                    | FeatureSubtype::MainStreet
                    | FeatureSubtype::Street
                    | FeatureSubtype::Path
                    | FeatureSubtype::RoadOther
            );

            if is_road {
                city_road_draws.push(draw);
            } else {
                city_draws.push(draw);
            }
        }

        // Ground plane
        let ground_size = 5000.0_f32;
        let ground_verts = vec![
            Vertex { position: [-ground_size, -0.3, -ground_size], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
            Vertex { position: [ ground_size, -0.3, -ground_size], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [ ground_size, -0.3,  ground_size], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [-ground_size, -0.3,  ground_size], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let ground_indices = vec![0, 1, 2, 0, 2, 3];
        let ground_cpu = CpuMesh { vertices: ground_verts, indices: ground_indices };
        let ground_gpu = GpuMesh::from_cpu(&self.renderer.device, &ground_cpu);
        let ground_mat = self.renderer.create_material([0.38, 0.65, 0.28, 1.0]);
        city_draws.insert(0, DrawCall { mesh: ground_gpu, material_bind_group: ground_mat });

        // City vegetation (occupancy grid)
        let mut city_instanced: Vec<InstancedDrawCall> = Vec::new();
        build_city_vegetation(&self.renderer, city_data, &mut city_instanced);

        self.draws = city_draws;
        self.road_draws = city_road_draws;
        self.instanced_draws = city_instanced;
        self.animated_draws.clear();
        self.flying_cpu.clear();
        self.water_draws.clear();
    }

    pub fn handle_cursor_move(&mut self, x: f32, y: f32) {
        self.cursor_pos = (x, y);
    }

    pub fn handle_click(&mut self) {
        if self.state != AppState::Riding {
            return;
        }
        let rects = camera_button_rects(
            self.renderer.width as f32,
            self.renderer.height as f32,
        );
        let (cx, cy) = self.cursor_pos;
        let modes = [CameraMode::ThirdPersonClose, CameraMode::ThirdPersonFar, CameraMode::FirstPerson];
        for (i, (x, y, w, h)) in rects.iter().enumerate() {
            if cx >= *x && cx <= x + w && cy >= *y && cy <= y + h {
                self.camera_mode = modes[i];
                return;
            }
        }
    }

    pub fn handle_touch(&mut self, touch: winit::event::Touch) {
        if touch.phase == TouchPhase::Ended {
            self.cursor_pos = (touch.location.x as f32, touch.location.y as f32);
            self.handle_click();
        }
    }
}

fn city_mesh_to_draw(renderer: &Renderer, cm: &CityMesh) -> DrawCall {
    let vertices: Vec<Vertex> = cm
        .vertices
        .iter()
        .zip(&cm.normals)
        .map(|(p, n)| Vertex {
            position: *p,
            normal: *n,
            uv: [0.0, 0.0],
        })
        .collect();
    let cpu = CpuMesh {
        vertices,
        indices: cm.indices.clone(),
    };
    let mesh = GpuMesh::from_cpu(&renderer.device, &cpu);
    let material = renderer.create_material(cm.color);
    DrawCall {
        mesh,
        material_bind_group: material,
    }
}

/// Build city vegetation using occupancy grid — port of world_render.rs:271-462
fn build_city_vegetation(
    renderer: &Renderer,
    city: &CityData,
    instanced_draws: &mut Vec<InstancedDrawCall>,
) {
    let road_network = match &city.road_network {
        Some(rn) => rn,
        None => return,
    };

    // 1. Compute city bounds from road nodes
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for node in &road_network.nodes {
        min_x = min_x.min(node.position[0]);
        max_x = max_x.max(node.position[0]);
        min_z = min_z.min(node.position[1]);
        max_z = max_z.max(node.position[1]);
    }
    let bound_pad = 40.0;
    min_x -= bound_pad;
    max_x += bound_pad;
    min_z -= bound_pad;
    max_z += bound_pad;

    // 2. Build occupancy grid
    let cell = 5.0_f32;
    let cols = ((max_x - min_x) / cell).ceil() as usize + 1;
    let rows = ((max_z - min_z) / cell).ceil() as usize + 1;
    if cols * rows > 2_000_000 {
        return;
    }
    let mut grid = vec![false; cols * rows];

    // Mark roads
    let road_buffer = 3.0_f32;
    for edge in &road_network.edges {
        let hw = edge.subtype.default_road_half_width() + road_buffer;
        for seg in edge.centerline.windows(2) {
            let [x0, z0] = seg[0];
            let [x1, z1] = seg[1];
            let dx = x1 - x0;
            let dz = z1 - z0;
            let len = (dx * dx + dz * dz).sqrt();
            if len < 0.01 {
                continue;
            }
            let nx = -dz / len;
            let nz = dx / len;
            let steps = (len / (cell * 0.5)).ceil() as usize;
            for s in 0..=steps {
                let t = s as f32 / steps.max(1) as f32;
                let cx = x0 + dx * t;
                let cz = z0 + dz * t;
                let perp_steps = (hw / (cell * 0.5)).ceil() as i32;
                for p in -perp_steps..=perp_steps {
                    let d = p as f32 * cell * 0.5;
                    mark_cell(
                        &mut grid,
                        cx + nx * d,
                        cz + nz * d,
                        min_x,
                        min_z,
                        cell,
                        cols,
                        rows,
                    );
                }
            }
        }
    }

    // Mark buildings, city roads, water
    for mesh in &city.buildings {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 1);
    }
    for mesh in &city.roads {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 1);
    }
    for mesh in &city.water {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 0);
    }

    // 3. Collect tree instances in empty cells
    let trunk_mesh = generate_trunk();
    let canopy_mesh = generate_canopy();
    let trunk_gpu = GpuMesh::from_cpu(&renderer.device, &trunk_mesh);

    let trunk_mat = renderer.create_material([0.35, 0.22, 0.10, 1.0]);
    let greens: [[f32; 4]; 4] = [
        [0.15, 0.48, 0.10, 1.0],
        [0.22, 0.55, 0.16, 1.0],
        [0.18, 0.58, 0.20, 1.0],
        [0.28, 0.52, 0.14, 1.0],
    ];

    // Group instances by green color variant for instanced rendering
    let mut trunk_instances: Vec<InstanceData> = Vec::new();
    let mut canopy_instances: [Vec<InstanceData>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

    for r in 0..rows {
        for c in 0..cols {
            if grid[r * cols + c] {
                continue;
            }

            let wx = min_x + (c as f32 + 0.5) * cell;
            let wz = min_z + (r as f32 + 0.5) * cell;
            let h = pos_hash(wx, wz);

            if h % 100 >= 25 {
                continue;
            }

            let green_idx = ((h / 3) % 4) as usize;
            let scale = 0.80 + (h % 350) as f32 / 700.0;

            let jx = ((h / 7) % 100) as f32 / 100.0 * cell * 0.8 - cell * 0.4;
            let jz = ((h / 11) % 100) as f32 / 100.0 * cell * 0.8 - cell * 0.4;
            let tx = wx + jx;
            let tz = wz + jz;

            // Trunk instance
            let trunk_y = 2.25 * scale;
            trunk_instances.push(InstanceData {
                position: [tx, trunk_y, tz],
                scale,
            });

            // Canopy instance
            let canopy_y = 5.2 * scale;
            canopy_instances[green_idx].push(InstanceData {
                position: [tx, canopy_y, tz],
                scale,
            });
        }
    }

    // Create instanced draw calls
    if !trunk_instances.is_empty() {
        let buf = renderer.create_instance_buffer(&trunk_instances);
        instanced_draws.push(InstancedDrawCall {
            mesh: trunk_gpu,
            instance_buffer: buf,
            instance_count: trunk_instances.len() as u32,
            material_bind_group: trunk_mat,
        });
    }

    for (i, instances) in canopy_instances.iter().enumerate() {
        if !instances.is_empty() {
            let canopy_gpu_copy = GpuMesh::from_cpu(&renderer.device, &canopy_mesh);
            let mat = renderer.create_material(greens[i]);
            let buf = renderer.create_instance_buffer(instances);
            instanced_draws.push(InstancedDrawCall {
                mesh: canopy_gpu_copy,
                instance_buffer: buf,
                instance_count: instances.len() as u32,
                material_bind_group: mat,
            });
        }
    }
}

fn mark_cell(
    grid: &mut [bool],
    x: f32,
    z: f32,
    origin_x: f32,
    origin_z: f32,
    cell: f32,
    cols: usize,
    rows: usize,
) {
    let c = ((x - origin_x) / cell) as i32;
    let r = ((z - origin_z) / cell) as i32;
    if c >= 0 && r >= 0 && (c as usize) < cols && (r as usize) < rows {
        grid[r as usize * cols + c as usize] = true;
    }
}

fn mark_mesh_triangles(
    grid: &mut [bool],
    mesh: &CityMesh,
    origin_x: f32,
    origin_z: f32,
    cell: f32,
    cols: usize,
    rows: usize,
    padding: i32,
) {
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = mesh.vertices[tri[0] as usize];
        let v1 = mesh.vertices[tri[1] as usize];
        let v2 = mesh.vertices[tri[2] as usize];

        let bmin_x = v0[0].min(v1[0]).min(v2[0]);
        let bmax_x = v0[0].max(v1[0]).max(v2[0]);
        let bmin_z = v0[2].min(v1[2]).min(v2[2]);
        let bmax_z = v0[2].max(v1[2]).max(v2[2]);

        let c0 = (((bmin_x - origin_x) / cell).floor() as i32 - padding).max(0);
        let c1 = (((bmax_x - origin_x) / cell).ceil() as i32 + padding).min(cols as i32 - 1);
        let r0 = (((bmin_z - origin_z) / cell).floor() as i32 - padding).max(0);
        let r1 = (((bmax_z - origin_z) / cell).ceil() as i32 + padding).min(rows as i32 - 1);

        for r in r0..=r1 {
            for c in c0..=c1 {
                grid[r as usize * cols + c as usize] = true;
            }
        }
    }
}

fn pos_hash(x: f32, z: f32) -> u32 {
    let a = (x * 73.856 + 19.143).to_bits();
    let b = (z * 37.292 + 47.857).to_bits();
    a.wrapping_mul(2654435761)
        .wrapping_add(b.wrapping_mul(2246822519))
}
