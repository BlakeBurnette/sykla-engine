use godot::classes::base_material_3d::Feature;
use godot::classes::mesh::ArrayType;
use godot::classes::{
    ArrayMesh, BoxMesh, CylinderMesh, Environment, MeshInstance3D, ProceduralSkyMaterial, Sky,
    SphereMesh, StandardMaterial3D, WorldEnvironment,
};
use godot::prelude::*;
use sykla_core::gradient::elevation_at_distance;
use sykla_core::types::RoutePoint;
use sykla_world::city::FeatureSubtype;
use sykla_world::road::{generate_road, RoadConfig};
use sykla_world::terrain::{generate_terrain, TerrainConfig};

use crate::route::SyklaRoute;

// ---------------------------------------------------------------------------
// Deterministic hash for procedural placement
// ---------------------------------------------------------------------------
fn phash(mut s: u64) -> u64 {
    s = s.wrapping_mul(6364136223846793005);
    s = s.wrapping_add(1442695040888963407);
    s
}
fn frand(state: &mut u64) -> f32 {
    *state = phash(*state);
    (*state % 10000) as f32 / 10000.0
}

// ---------------------------------------------------------------------------
// SyklaWorldGenerator
// ---------------------------------------------------------------------------
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SyklaWorldGenerator {
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for SyklaWorldGenerator {
    fn init(base: Base<Node3D>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl SyklaWorldGenerator {
    #[func]
    fn generate_world(&mut self, route: Gd<SyklaRoute>) {
        let route_ref = route.bind();
        let inner = match route_ref.inner() {
            Some(r) => r,
            None => {
                godot_error!("SyklaWorldGenerator: route has no data loaded");
                return;
            }
        };

        self.clear_children();
        self.setup_environment();

        // Terrain (flattened for city — less falloff, wider)
        let config = TerrainConfig {
            half_width: 80.0,
            cross_sections: 10,
            falloff: 0.05,
            elevation_scale: 1.0,
        };
        let terrain_mesh = generate_terrain(&inner.points, &config);
        if !terrain_mesh.vertices.is_empty() {
            let mut gm = build_array_mesh(
                &terrain_mesh.vertices,
                &terrain_mesh.normals,
                &terrain_mesh.indices,
            );
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.28, 0.52, 0.18));
            mat.set_roughness(0.92);
            mat.set_metallic(0.0);
            gm.surface_set_material(0, &mat);

            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&gm);
            inst.set_name("Terrain");
            self.base_mut().add_child(&inst);
        }

        // Main road
        let road_mesh = generate_road(&inner.points, &RoadConfig::default());
        if !road_mesh.vertices.is_empty() {
            let mut gm = build_array_mesh(
                &road_mesh.vertices,
                &road_mesh.normals,
                &road_mesh.indices,
            );
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.15, 0.15, 0.17));
            mat.set_roughness(0.82);
            mat.set_specular(0.35);
            gm.surface_set_material(0, &mat);

            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&gm);
            inst.set_name("Road");
            self.base_mut().add_child(&inst);
        }

        // Road markings (center dashed yellow + edge white)
        self.spawn_center_line(&inner.points);
        self.spawn_road_lines(&inner.points);

        // Sidewalks on both sides
        self.spawn_sidewalks(&inner.points);

        // City blocks with buildings
        self.spawn_city_blocks(&inner.points);

        // Street trees
        self.spawn_street_trees(&inner.points);

        // Cross-streets
        self.spawn_cross_streets(&inner.points);
    }

    #[func]
    fn clear_children(&mut self) {
        let children = self.base().get_children();
        for child in children.iter_shared() {
            let mut child = child;
            child.queue_free();
        }
    }

    // -----------------------------------------------------------------------
    // Environment
    // -----------------------------------------------------------------------
    fn setup_environment(&mut self) {
        let mut sky_mat = ProceduralSkyMaterial::new_gd();
        sky_mat.set_sky_top_color(Color::from_rgb(0.32, 0.52, 0.82));
        sky_mat.set_sky_horizon_color(Color::from_rgb(0.62, 0.76, 0.90));
        sky_mat.set_ground_bottom_color(Color::from_rgb(0.22, 0.26, 0.20));
        sky_mat.set_ground_horizon_color(Color::from_rgb(0.52, 0.62, 0.50));
        sky_mat.set_sky_energy_multiplier(1.05);

        let mut sky = Sky::new_gd();
        sky.set_material(&sky_mat);

        let mut env = Environment::new_gd();
        env.set_background(godot::classes::environment::BgMode::SKY);
        env.set_sky(&sky);
        env.set_ambient_source(godot::classes::environment::AmbientSource::SKY);
        env.set_ambient_light_energy(0.5);
        env.set_ambient_light_color(Color::from_rgb(0.75, 0.82, 0.92));
        env.set_tonemapper(godot::classes::environment::ToneMapper::ACES);
        env.set_ssao_enabled(true);
        env.set_ssao_intensity(1.8);
        env.set_ssao_radius(2.5);
        env.set_ssil_enabled(true);
        env.set_ssil_intensity(1.0);
        env.set_glow_enabled(true);
        env.set_glow_intensity(0.3);
        env.set_glow_bloom(0.04);
        env.set_fog_enabled(true);
        env.set_fog_light_color(Color::from_rgb(0.70, 0.78, 0.86));
        env.set_fog_density(0.003);
        env.set_fog_sky_affect(0.5);

        let mut we = WorldEnvironment::new_alloc();
        we.set_environment(&env);
        we.set_name("WorldEnvironment");
        self.base_mut().add_child(&we);
    }

    // -----------------------------------------------------------------------
    // City blocks with buildings + detail
    // -----------------------------------------------------------------------
    fn spawn_city_blocks(&mut self, points: &[RoutePoint]) {
        if points.len() < 2 {
            return;
        }
        let total_dist = points.last().unwrap().distance_from_start_m as f32;
        let block_spacing = 18.0f32;
        let mut rng = 12345u64;

        let building_types = [
            FeatureSubtype::Residential,
            FeatureSubtype::Commercial,
            FeatureSubtype::Retail,
            FeatureSubtype::Public,
            FeatureSubtype::Industrial,
        ];

        // Neon sign colors (bright, emissive)
        let sign_colors: &[[f32; 3]] = &[
            [1.0, 0.2, 0.3],   // hot pink
            [0.2, 0.6, 1.0],   // electric blue
            [1.0, 0.85, 0.1],  // golden yellow
            [0.1, 1.0, 0.4],   // neon green
            [1.0, 0.4, 0.1],   // orange
            [0.8, 0.2, 1.0],   // purple
        ];

        // Graffiti palette (bright street art colors)
        let graffiti_colors: &[[f32; 3]] = &[
            [0.9, 0.15, 0.2],  // red
            [0.1, 0.5, 0.95],  // blue
            [1.0, 0.75, 0.0],  // yellow
            [0.0, 0.8, 0.35],  // green
            [0.85, 0.3, 0.9],  // magenta
            [1.0, 0.45, 0.0],  // orange
            [0.0, 0.9, 0.9],   // cyan
        ];

        // Awning colors
        let awning_colors: &[[f32; 3]] = &[
            [0.6, 0.12, 0.12],  // burgundy
            [0.12, 0.35, 0.12], // forest green
            [0.15, 0.15, 0.45], // navy
            [0.55, 0.35, 0.10], // brown
            [0.7, 0.55, 0.15],  // gold
        ];

        let n_blocks = (total_dist / block_spacing) as usize;

        for i in 0..n_blocks {
            let z = i as f32 * block_spacing + block_spacing * 0.5;
            let base_y = elevation_at_distance(points, z as f64) as f32;

            for side in [-1.0f32, 1.0] {
                if frand(&mut rng) < 0.10 {
                    continue;
                }

                let btype_idx = (phash(rng) % building_types.len() as u64) as usize;
                let btype = building_types[btype_idx];
                let color = btype.default_color();

                let base_height = btype.default_height();
                let height = base_height * (0.6 + frand(&mut rng) * 0.9);
                let width = 6.0 + frand(&mut rng) * 8.0;
                let depth = 8.0 + frand(&mut rng) * 10.0;
                let setback = 7.0 + frand(&mut rng) * 4.0;
                let x = side * (setback + width * 0.5);
                let sl = if side > 0.0 { "R" } else { "L" };

                // Main body
                self.spawn_building_box(
                    x, base_y, z, width, height, depth,
                    Color::from_rgb(color[0], color[1], color[2]),
                    0.75 + frand(&mut rng) * 0.2,
                    &format!("Bldg_{i}_{sl}"),
                );

                // Floor cornices — thin horizontal bands between floors
                let floors = (height / 3.2) as usize;
                for f in 1..floors {
                    let cy = base_y + f as f32 * 3.2;
                    let face_x = x - side * (width * 0.5 + 0.06);
                    let mut bm = BoxMesh::new_gd();
                    bm.set_size(Vector3::new(0.12, 0.25, width + 0.2));
                    let mut mat = StandardMaterial3D::new_gd();
                    let cc = FeatureSubtype::Cornice.default_color();
                    mat.set_albedo(Color::from_rgb(cc[0], cc[1], cc[2]));
                    mat.set_roughness(0.80);
                    bm.surface_set_material(0, &mat);
                    let mut inst = MeshInstance3D::new_alloc();
                    inst.set_mesh(&bm);
                    inst.set_position(Vector3::new(face_x, cy, z));
                    inst.set_name(&format!("Corn_{i}_{f}_{sl}"));
                    self.base_mut().add_child(&inst);
                }

                // Windows
                let wpf = (width / 2.2) as usize;
                if floors > 0 && wpf > 0 {
                    self.spawn_windows(
                        x, base_y, z, width, height, depth,
                        floors, wpf, side,
                        &format!("Win_{i}_{sl}"),
                    );
                }

                // Ground-floor storefront awning (retail/commercial)
                let is_commercial = matches!(
                    btype,
                    FeatureSubtype::Commercial | FeatureSubtype::Retail
                );
                if is_commercial || frand(&mut rng) < 0.35 {
                    let ac = awning_colors[(phash(rng) % awning_colors.len() as u64) as usize];
                    let awning_x = x - side * (width * 0.5 + 0.8);
                    let mut bm = BoxMesh::new_gd();
                    bm.set_size(Vector3::new(1.6, 0.08, width * 0.7));
                    let mut mat = StandardMaterial3D::new_gd();
                    mat.set_albedo(Color::from_rgb(ac[0], ac[1], ac[2]));
                    mat.set_roughness(0.70);
                    bm.surface_set_material(0, &mat);
                    let mut inst = MeshInstance3D::new_alloc();
                    inst.set_mesh(&bm);
                    inst.set_position(Vector3::new(awning_x, base_y + 3.2, z));
                    inst.set_name(&format!("Awning_{i}_{sl}"));
                    self.base_mut().add_child(&inst);

                    // Storefront glass below awning
                    let sf_x = x - side * (width * 0.5 + 0.04);
                    let mut sfm = BoxMesh::new_gd();
                    sfm.set_size(Vector3::new(0.06, 2.4, width * 0.65));
                    let sfc = FeatureSubtype::Storefront.default_color();
                    let mut sfmat = StandardMaterial3D::new_gd();
                    sfmat.set_albedo(Color::from_rgb(sfc[0], sfc[1], sfc[2]));
                    sfmat.set_roughness(0.1);
                    sfmat.set_metallic(0.5);
                    sfmat.set_specular(0.9);
                    sfm.surface_set_material(0, &sfmat);
                    let mut sfi = MeshInstance3D::new_alloc();
                    sfi.set_mesh(&sfm);
                    sfi.set_position(Vector3::new(sf_x, base_y + 1.5, z));
                    sfi.set_name(&format!("Store_{i}_{sl}"));
                    self.base_mut().add_child(&sfi);
                }

                // Neon / illuminated sign (on facade, ~2nd-3rd floor)
                if frand(&mut rng) < 0.45 {
                    let sc = sign_colors[(phash(rng) % sign_colors.len() as u64) as usize];
                    let sign_y = base_y + 4.5 + frand(&mut rng) * 3.0;
                    let sign_w = 1.5 + frand(&mut rng) * 2.5;
                    let sign_h = 0.6 + frand(&mut rng) * 1.0;
                    let sign_x = x - side * (width * 0.5 + 0.08);
                    let sign_z = z + (frand(&mut rng) - 0.5) * width * 0.4;

                    let mut bm = BoxMesh::new_gd();
                    bm.set_size(Vector3::new(0.06, sign_h, sign_w));
                    let mut mat = StandardMaterial3D::new_gd();
                    mat.set_albedo(Color::from_rgb(sc[0], sc[1], sc[2]));
                    mat.set_feature(Feature::EMISSION, true);
                    mat.set_emission(Color::from_rgb(sc[0], sc[1], sc[2]));
                    mat.set_emission_energy_multiplier(3.5);
                    mat.set_roughness(0.3);
                    bm.surface_set_material(0, &mat);
                    let mut inst = MeshInstance3D::new_alloc();
                    inst.set_mesh(&bm);
                    inst.set_position(Vector3::new(sign_x, sign_y, sign_z));
                    inst.set_name(&format!("Sign_{i}_{sl}"));
                    self.base_mut().add_child(&inst);

                    // Sign backing panel (dark)
                    let mut bp = BoxMesh::new_gd();
                    bp.set_size(Vector3::new(0.04, sign_h + 0.3, sign_w + 0.3));
                    let mut bpm = StandardMaterial3D::new_gd();
                    bpm.set_albedo(Color::from_rgb(0.08, 0.08, 0.10));
                    bpm.set_roughness(0.4);
                    bp.surface_set_material(0, &bpm);
                    let mut bpi = MeshInstance3D::new_alloc();
                    bpi.set_mesh(&bp);
                    bpi.set_position(Vector3::new(
                        sign_x + side * 0.03, sign_y, sign_z,
                    ));
                    bpi.set_name(&format!("SignBk_{i}_{sl}"));
                    self.base_mut().add_child(&bpi);
                }

                // Billboard ad (higher up on taller buildings)
                if height > 9.0 && frand(&mut rng) < 0.3 {
                    let bc = sign_colors[(phash(rng.wrapping_add(7)) % sign_colors.len() as u64) as usize];
                    let bb_y = base_y + height - 3.0;
                    let bb_x = x - side * (width * 0.5 + 0.10);
                    let bb_w = width * 0.6 + frand(&mut rng) * 2.0;
                    let bb_h = 2.0 + frand(&mut rng) * 1.5;

                    let mut bm = BoxMesh::new_gd();
                    bm.set_size(Vector3::new(0.08, bb_h, bb_w));
                    let mut mat = StandardMaterial3D::new_gd();
                    // Slightly muted emissive billboard
                    mat.set_albedo(Color::from_rgb(bc[0] * 0.8, bc[1] * 0.8, bc[2] * 0.8));
                    mat.set_feature(Feature::EMISSION, true);
                    mat.set_emission(Color::from_rgb(bc[0] * 0.5, bc[1] * 0.5, bc[2] * 0.5));
                    mat.set_emission_energy_multiplier(1.5);
                    mat.set_roughness(0.5);
                    bm.surface_set_material(0, &mat);
                    let mut inst = MeshInstance3D::new_alloc();
                    inst.set_mesh(&bm);
                    inst.set_position(Vector3::new(bb_x, bb_y, z));
                    inst.set_name(&format!("BB_{i}_{sl}"));
                    self.base_mut().add_child(&inst);
                }

                // Graffiti tags (street-level colored patches)
                if frand(&mut rng) < 0.35 {
                    let n_tags = 1 + (phash(rng) % 3) as usize;
                    for t in 0..n_tags {
                        let gc = graffiti_colors[
                            (phash(rng.wrapping_add(t as u64 * 31)) % graffiti_colors.len() as u64) as usize
                        ];
                        let tag_y = base_y + 0.8 + frand(&mut rng) * 2.0;
                        let tag_z = z + (frand(&mut rng) - 0.5) * width * 0.6;
                        let tag_w = 0.8 + frand(&mut rng) * 1.8;
                        let tag_h = 0.5 + frand(&mut rng) * 1.2;
                        let tag_x = x - side * (width * 0.5 + 0.04);

                        let mut bm = BoxMesh::new_gd();
                        bm.set_size(Vector3::new(0.03, tag_h, tag_w));
                        let mut mat = StandardMaterial3D::new_gd();
                        mat.set_albedo(Color::from_rgb(gc[0], gc[1], gc[2]));
                        mat.set_roughness(0.60);
                        mat.set_metallic(0.0);
                        bm.surface_set_material(0, &mat);
                        let mut inst = MeshInstance3D::new_alloc();
                        inst.set_mesh(&bm);
                        inst.set_position(Vector3::new(tag_x, tag_y, tag_z));
                        inst.set_name(&format!("Graf_{i}_{t}_{sl}"));
                        self.base_mut().add_child(&inst);
                    }
                }

                // Rooftop water tank (cylinder on tall buildings)
                if height > 8.0 && frand(&mut rng) < 0.35 {
                    let tank_r = 0.6 + frand(&mut rng) * 0.5;
                    let tank_h = 1.5 + frand(&mut rng) * 1.0;
                    let tank_x = x + (frand(&mut rng) - 0.5) * width * 0.3;
                    let tank_z = z + (frand(&mut rng) - 0.5) * depth * 0.3;

                    // Legs
                    let leg_y = base_y + height + 0.8;
                    let mut leg = CylinderMesh::new_gd();
                    leg.set_top_radius(0.06);
                    leg.set_bottom_radius(0.06);
                    leg.set_height(1.6);
                    leg.set_radial_segments(4);
                    let mut lm = StandardMaterial3D::new_gd();
                    lm.set_albedo(Color::from_rgb(0.35, 0.35, 0.38));
                    lm.set_metallic(0.6);
                    lm.set_roughness(0.4);
                    leg.surface_set_material(0, &lm);

                    for lx in [-0.4f32, 0.4] {
                        for lz in [-0.4f32, 0.4] {
                            let mut li = MeshInstance3D::new_alloc();
                            li.set_mesh(&leg);
                            li.set_position(Vector3::new(
                                tank_x + lx * tank_r,
                                leg_y,
                                tank_z + lz * tank_r,
                            ));
                            li.set_name(&format!("TLeg_{i}"));
                            self.base_mut().add_child(&li);
                        }
                    }

                    // Tank body
                    let mut tank = CylinderMesh::new_gd();
                    tank.set_top_radius(tank_r);
                    tank.set_bottom_radius(tank_r);
                    tank.set_height(tank_h);
                    tank.set_radial_segments(10);
                    let mut tm = StandardMaterial3D::new_gd();
                    tm.set_albedo(Color::from_rgb(0.40, 0.30, 0.18));
                    tm.set_roughness(0.90);
                    tank.surface_set_material(0, &tm);
                    let mut ti = MeshInstance3D::new_alloc();
                    ti.set_mesh(&tank);
                    ti.set_position(Vector3::new(
                        tank_x,
                        base_y + height + 1.6 + tank_h * 0.5,
                        tank_z,
                    ));
                    ti.set_name(&format!("Tank_{i}_{sl}"));
                    self.base_mut().add_child(&ti);

                    // Conical roof
                    let mut roof = CylinderMesh::new_gd();
                    roof.set_top_radius(0.05);
                    roof.set_bottom_radius(tank_r + 0.1);
                    roof.set_height(0.5);
                    roof.set_radial_segments(10);
                    let mut rm = StandardMaterial3D::new_gd();
                    rm.set_albedo(Color::from_rgb(0.30, 0.30, 0.32));
                    rm.set_metallic(0.4);
                    rm.set_roughness(0.5);
                    roof.surface_set_material(0, &rm);
                    let mut ri = MeshInstance3D::new_alloc();
                    ri.set_mesh(&roof);
                    ri.set_position(Vector3::new(
                        tank_x,
                        base_y + height + 1.6 + tank_h + 0.25,
                        tank_z,
                    ));
                    ri.set_name(&format!("TRoof_{i}_{sl}"));
                    self.base_mut().add_child(&ri);
                }

                // AC units on facade
                if frand(&mut rng) < 0.4 && floors > 1 {
                    let n_ac = 1 + (phash(rng) % 3) as usize;
                    for a in 0..n_ac {
                        let ac_floor = 1 + (phash(rng.wrapping_add(a as u64 * 13)) % (floors as u64).max(1)) as usize;
                        let ac_y = base_y + ac_floor as f32 * 3.2 - 0.5;
                        let ac_z = z + (frand(&mut rng) - 0.5) * width * 0.5;
                        let ac_x = x - side * (width * 0.5 + 0.25);

                        let mut bm = BoxMesh::new_gd();
                        bm.set_size(Vector3::new(0.5, 0.4, 0.6));
                        let mut mat = StandardMaterial3D::new_gd();
                        mat.set_albedo(Color::from_rgb(0.75, 0.75, 0.72));
                        mat.set_roughness(0.6);
                        mat.set_metallic(0.3);
                        bm.surface_set_material(0, &mat);
                        let mut inst = MeshInstance3D::new_alloc();
                        inst.set_mesh(&bm);
                        inst.set_position(Vector3::new(ac_x, ac_y, ac_z));
                        inst.set_name(&format!("AC_{i}_{a}_{sl}"));
                        self.base_mut().add_child(&inst);
                    }
                }

                // Simple rooftop box (mechanical / elevator)
                if height > 10.0 && frand(&mut rng) < 0.4 {
                    let rw = width * 0.3;
                    let rh = height * 0.12;
                    self.spawn_building_box(
                        x, base_y + height, z, rw, rh, rw,
                        Color::from_rgb(0.48, 0.50, 0.53), 0.45,
                        &format!("Mech_{i}_{sl}"),
                    );
                }
            }
        }
    }

    fn spawn_building_box(
        &mut self,
        x: f32, base_y: f32, z: f32,
        w: f32, h: f32, d: f32,
        color: Color,
        roughness: f32,
        name: &str,
    ) {
        let mut bm = BoxMesh::new_gd();
        bm.set_size(Vector3::new(w, h, d));

        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(color);
        mat.set_roughness(roughness);
        mat.set_metallic(0.0);
        mat.set_specular(0.25);
        bm.surface_set_material(0, &mat);

        let mut inst = MeshInstance3D::new_alloc();
        inst.set_mesh(&bm);
        inst.set_position(Vector3::new(x, base_y + h * 0.5, z));
        inst.set_name(name);
        self.base_mut().add_child(&inst);
    }

    fn spawn_windows(
        &mut self,
        bx: f32, base_y: f32, bz: f32,
        bw: f32, bh: f32, _bd: f32,
        floors: usize, per_floor: usize,
        side: f32,
        name: &str,
    ) {
        let win_w = 1.2f32;
        let win_h = 1.6f32;
        let win_depth = 0.08;
        let floor_h = bh / floors as f32;

        // Glass material
        let mut glass_mat = StandardMaterial3D::new_gd();
        let gc = FeatureSubtype::WindowGlass.default_color();
        glass_mat.set_albedo(Color::from_rgb(gc[0], gc[1], gc[2]));
        glass_mat.set_roughness(0.15);
        glass_mat.set_metallic(0.6);
        glass_mat.set_specular(0.9);

        // Batch all windows for this building into one mesh to reduce draw calls
        let mut verts = Vec::new();
        let mut norms = Vec::new();
        let mut idxs = Vec::new();

        // Face normal pointing toward the road (inward toward X=0)
        let nx = -side;

        // X position of the window face (on the side facing the road)
        let face_x = bx - side * (bw * 0.5 + win_depth * 0.5);

        for floor in 0..floors {
            let y_center = base_y + floor as f32 * floor_h + floor_h * 0.6;
            for wi in 0..per_floor {
                let z_center = bz - bw * 0.5 + (wi as f32 + 0.5) * (bw / per_floor as f32);

                let base_idx = verts.len() as u32;
                // Quad facing road
                let hw = win_w * 0.5;
                let hh = win_h * 0.5;
                verts.push([face_x, y_center - hh, z_center - hw]);
                verts.push([face_x, y_center - hh, z_center + hw]);
                verts.push([face_x, y_center + hh, z_center + hw]);
                verts.push([face_x, y_center + hh, z_center - hw]);
                for _ in 0..4 {
                    norms.push([nx, 0.0, 0.0]);
                }
                idxs.extend_from_slice(&[
                    base_idx, base_idx + 1, base_idx + 2,
                    base_idx, base_idx + 2, base_idx + 3,
                ]);
            }
        }

        if !verts.is_empty() {
            let mut gm = build_array_mesh_ccw(&verts, &norms, &idxs);
            gm.surface_set_material(0, &glass_mat);
            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&gm);
            inst.set_name(name);
            self.base_mut().add_child(&inst);
        }
    }

    // -----------------------------------------------------------------------
    // Sidewalks
    // -----------------------------------------------------------------------
    fn spawn_sidewalks(&mut self, points: &[RoutePoint]) {
        for side in [-4.5f32, 4.5] {
            let sw = 1.8f32; // sidewalk width
            let n = points.len();
            if n < 2 {
                return;
            }

            let mut vs = Vec::with_capacity(n * 2);
            let mut ns = Vec::with_capacity(n * 2);
            let mut is = Vec::new();

            for p in points.iter() {
                let z = p.distance_from_start_m as f32;
                let y = p.elevation_m as f32 + 0.12; // slight curb
                vs.push([side - sw * 0.5, y, z]);
                vs.push([side + sw * 0.5, y, z]);
                ns.push([0.0f32, 1.0, 0.0]);
                ns.push([0.0f32, 1.0, 0.0]);
            }
            for i in 0..n - 1 {
                let b = (i * 2) as u32;
                is.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
            }

            let mut gm = build_array_mesh(&vs, &ns, &is);
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.72, 0.70, 0.66));
            mat.set_roughness(0.88);
            gm.surface_set_material(0, &mat);

            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&gm);
            inst.set_name(&format!("Sidewalk_{}", if side < 0.0 { "L" } else { "R" }));
            self.base_mut().add_child(&inst);
        }
    }

    // -----------------------------------------------------------------------
    // Cross-streets every ~60m
    // -----------------------------------------------------------------------
    fn spawn_cross_streets(&mut self, points: &[RoutePoint]) {
        if points.len() < 2 {
            return;
        }
        let total = points.last().unwrap().distance_from_start_m as f32;
        let spacing = 55.0f32;
        let road_w = 6.0f32;
        let n = (total / spacing) as usize;

        for i in 1..n {
            let z = i as f32 * spacing;
            let y = elevation_at_distance(points, z as f64) as f32 + 0.06;
            let half_len = 75.0f32;

            let mut bm = BoxMesh::new_gd();
            bm.set_size(Vector3::new(half_len * 2.0, 0.05, road_w));
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.20, 0.20, 0.22));
            mat.set_roughness(0.82);
            bm.surface_set_material(0, &mat);

            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&bm);
            inst.set_position(Vector3::new(0.0, y, z));
            inst.set_name(&format!("CrossSt_{i}"));
            self.base_mut().add_child(&inst);
        }
    }

    // -----------------------------------------------------------------------
    // Street trees in tree wells along sidewalk
    // -----------------------------------------------------------------------
    fn spawn_street_trees(&mut self, points: &[RoutePoint]) {
        if points.len() < 2 {
            return;
        }
        let total = points.last().unwrap().distance_from_start_m as f32;
        let spacing = 12.0f32;
        let n = (total / spacing) as usize;
        let mut rng = 77777u64;

        for i in 0..n {
            let z = i as f32 * spacing + 6.0;
            let y = elevation_at_distance(points, z as f64) as f32;

            for side in [-5.5f32, 5.5] {
                if frand(&mut rng) < 0.2 {
                    continue; // occasional gap
                }

                let scale = 0.7 + frand(&mut rng) * 0.5;
                let x = side + (frand(&mut rng) - 0.5) * 0.5;

                // Trunk
                let mut trunk = CylinderMesh::new_gd();
                trunk.set_top_radius(0.08 * scale);
                trunk.set_bottom_radius(0.14 * scale);
                trunk.set_height(3.5 * scale);
                trunk.set_radial_segments(6);
                let mut tm = StandardMaterial3D::new_gd();
                tm.set_albedo(Color::from_rgb(0.32, 0.20, 0.10));
                tm.set_roughness(0.95);
                trunk.surface_set_material(0, &tm);

                let mut ti = MeshInstance3D::new_alloc();
                ti.set_mesh(&trunk);
                ti.set_position(Vector3::new(x, y + 1.75 * scale, z));
                ti.set_name(&format!("STrunk_{i}_{}", if side > 0.0 { "R" } else { "L" }));
                self.base_mut().add_child(&ti);

                // Canopy
                let mut sphere = SphereMesh::new_gd();
                sphere.set_radius(1.8 * scale);
                sphere.set_height(2.8 * scale);
                sphere.set_radial_segments(10);
                sphere.set_rings(6);
                let mut cm = StandardMaterial3D::new_gd();
                let g = 0.15 + frand(&mut rng) * 0.12;
                cm.set_albedo(Color::from_rgb(g, 0.42 + frand(&mut rng) * 0.15, 0.10));
                cm.set_roughness(0.85);
                sphere.surface_set_material(0, &cm);

                let mut ci = MeshInstance3D::new_alloc();
                ci.set_mesh(&sphere);
                ci.set_position(Vector3::new(x, y + 4.5 * scale, z));
                ci.set_name(&format!("SCanopy_{i}_{}", if side > 0.0 { "R" } else { "L" }));
                self.base_mut().add_child(&ci);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Road markings
    // -----------------------------------------------------------------------
    fn spawn_road_lines(&mut self, points: &[RoutePoint]) {
        for side in [-2.8f32, 2.8] {
            let n = points.len();
            if n < 2 {
                return;
            }
            let mut vs = Vec::with_capacity(n * 2);
            let mut ns = Vec::with_capacity(n * 2);
            let mut is = Vec::new();
            let hw = 0.06f32;
            for p in points.iter() {
                let z = p.distance_from_start_m as f32;
                let y = p.elevation_m as f32 + 0.07;
                vs.push([side - hw, y, z]);
                vs.push([side + hw, y, z]);
                ns.push([0.0f32, 1.0, 0.0]);
                ns.push([0.0f32, 1.0, 0.0]);
            }
            for i in 0..n - 1 {
                let b = (i * 2) as u32;
                is.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
            }
            let mut gm = build_array_mesh(&vs, &ns, &is);
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.92, 0.92, 0.90));
            mat.set_roughness(0.65);
            gm.surface_set_material(0, &mat);
            let mut inst = MeshInstance3D::new_alloc();
            inst.set_mesh(&gm);
            inst.set_name(&format!("EdgeLine_{}", if side < 0.0 { "L" } else { "R" }));
            self.base_mut().add_child(&inst);
        }
    }

    fn spawn_center_line(&mut self, points: &[RoutePoint]) {
        let n = points.len();
        if n < 2 {
            return;
        }
        let mut vs = Vec::with_capacity(n * 2);
        let mut ns = Vec::with_capacity(n * 2);
        let mut is = Vec::new();
        let hw = 0.05f32;
        for p in points.iter() {
            let z = p.distance_from_start_m as f32;
            let y = p.elevation_m as f32 + 0.07;
            vs.push([-hw, y, z]);
            vs.push([hw, y, z]);
            ns.push([0.0f32, 1.0, 0.0]);
            ns.push([0.0f32, 1.0, 0.0]);
        }
        for i in 0..n - 1 {
            let b = (i * 2) as u32;
            is.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
        }
        let mut gm = build_array_mesh(&vs, &ns, &is);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgb(0.90, 0.75, 0.10));
        mat.set_roughness(0.65);
        gm.surface_set_material(0, &mat);
        let mut inst = MeshInstance3D::new_alloc();
        inst.set_mesh(&gm);
        inst.set_name("CenterLine");
        self.base_mut().add_child(&inst);
    }
}

// ---------------------------------------------------------------------------
// Mesh helpers
// ---------------------------------------------------------------------------

/// Build ArrayMesh, reversing winding (for sykla-world CW data → Godot CCW).
fn build_array_mesh(
    vertices: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
) -> Gd<ArrayMesh> {
    let mut mesh = ArrayMesh::new_gd();
    let mut verts = PackedVector3Array::new();
    for v in vertices {
        verts.push(Vector3::new(v[0], v[1], v[2]));
    }
    let mut norms = PackedVector3Array::new();
    for n in normals {
        norms.push(Vector3::new(n[0], n[1], n[2]));
    }
    let mut idx = PackedInt32Array::new();
    for tri in indices.chunks(3) {
        idx.push(tri[0] as i32);
        idx.push(tri[2] as i32);
        idx.push(tri[1] as i32);
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &verts.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &norms.to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
    mesh.add_surface_from_arrays(godot::classes::mesh::PrimitiveType::TRIANGLES, &arrays);
    mesh
}

/// Build ArrayMesh keeping winding as-is (for data already in CCW order).
fn build_array_mesh_ccw(
    vertices: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
) -> Gd<ArrayMesh> {
    let mut mesh = ArrayMesh::new_gd();
    let mut verts = PackedVector3Array::new();
    for v in vertices {
        verts.push(Vector3::new(v[0], v[1], v[2]));
    }
    let mut norms = PackedVector3Array::new();
    for n in normals {
        norms.push(Vector3::new(n[0], n[1], n[2]));
    }
    let mut idx = PackedInt32Array::new();
    for &val in indices {
        idx.push(val as i32);
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &verts.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &norms.to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
    mesh.add_surface_from_arrays(godot::classes::mesh::PrimitiveType::TRIANGLES, &arrays);
    mesh
}
