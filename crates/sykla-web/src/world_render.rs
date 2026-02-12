use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use sykla_core::projection::LocalProjection;
use sykla_core::types::Route;
use sykla_world::{
    city::{CityData, CityMesh, FeatureSubtype},
    road::{generate_road, RoadConfig},
    road_network::RoadNetwork,
    terrain::{generate_terrain, TerrainConfig},
    vegetation::{generate_vegetation, VegetationConfig, VegetationType},
};

use crate::ride::ActiveRide;

/// Event to trigger world generation from a route.
#[derive(Event)]
pub struct GenerateWorldEvent {
    pub route: Route,
    /// Optional city data to render alongside the route.
    pub city_data: Option<CityData>,
}

/// Marker component for terrain entities.
#[derive(Component)]
pub struct TerrainEntity;

/// Marker component for road entities.
#[derive(Component)]
pub struct RoadEntity;

/// Marker component for tree entities.
#[derive(Component)]
pub struct TreeEntity;

/// Marker component for city geometry (buildings, landuse, water).
#[derive(Component)]
pub struct CityEntity;

/// Marker component for the rider avatar.
#[derive(Component)]
pub struct RiderAvatar;

/// Resource holding the projection for the current world.
#[derive(Resource)]
pub struct WorldProjection {
    pub projection: LocalProjection,
}

/// Resource holding the loaded road network for navigation.
#[derive(Resource)]
pub struct LoadedRoadNetwork {
    pub network: RoadNetwork,
}

/// Plugin for 3D world rendering.
pub struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GenerateWorldEvent>()
            .add_systems(Update, (generate_world_system, update_camera_system));
    }
}

/// System that generates the 3D world when a route is loaded.
fn generate_world_system(
    mut commands: Commands,
    mut events: EventReader<GenerateWorldEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain_query: Query<Entity, With<TerrainEntity>>,
    road_query: Query<Entity, With<RoadEntity>>,
    tree_query: Query<Entity, With<TreeEntity>>,
    city_query: Query<Entity, With<CityEntity>>,
    rider_query: Query<Entity, With<RiderAvatar>>,
) {
    for event in events.read() {
        // Despawn existing world entities
        for entity in terrain_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for entity in road_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for entity in tree_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for entity in city_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for entity in rider_query.iter() {
            commands.entity(entity).despawn_recursive();
        }

        let route = &event.route;
        let points = &route.points;

        // If city data is available, use projected coordinates;
        // otherwise fall back to the original route-relative system.
        let has_city = event.city_data.is_some();

        // Set up projection if we have city data
        if let Some(ref city) = event.city_data {
            let proj = LocalProjection::new(city.center_lat, city.center_lng);
            commands.insert_resource(WorldProjection { projection: proj });
        }

        // Generate terrain
        let terrain_config = TerrainConfig::default();
        let terrain_mesh = generate_terrain(points, &terrain_config);
        if !terrain_mesh.vertices.is_empty() {
            let mesh = mesh_from_world_data(
                &terrain_mesh.vertices,
                &terrain_mesh.normals,
                &terrain_mesh.uvs,
                &terrain_mesh.indices,
            );
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.7, 0.2),
                    perceptual_roughness: 0.9,
                    ..default()
                })),
                TerrainEntity,
            ));
        }

        // Generate road
        let road_config = RoadConfig {
            elevation_scale: terrain_config.elevation_scale,
            ..Default::default()
        };
        let road_mesh_data = generate_road(points, &road_config);
        if !road_mesh_data.vertices.is_empty() {
            let mesh = mesh_from_world_data(
                &road_mesh_data.vertices,
                &road_mesh_data.normals,
                &road_mesh_data.uvs,
                &road_mesh_data.indices,
            );
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.3, 0.35),
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                RoadEntity,
            ));
        }

        // Generate vegetation (only when no city data — city data provides its own parks/trees)
        if !has_city {
            let veg_config = VegetationConfig {
                elevation_scale: terrain_config.elevation_scale,
                ..Default::default()
            };
            let vegetation = generate_vegetation(points, &veg_config);
            spawn_vegetation(&mut commands, &mut meshes, &mut materials, &vegetation);
        }

        // Spawn city geometry if available
        if let Some(ref city) = event.city_data {
            spawn_city_data(&mut commands, &mut meshes, &mut materials, city);

            // Fill empty space with vegetation (Waze-style: fill everything, subtract roads/buildings/water)
            spawn_city_vegetation(&mut commands, &mut meshes, &mut materials, city);

            // Insert road network resource for navigation
            if let Some(ref rn) = city.road_network {
                commands.insert_resource(LoadedRoadNetwork {
                    network: rn.clone(),
                });
            }

            // Ground plane — large flat green surface under the city
            let ground_size = 5000.0; // 5 km each direction
            commands.spawn((
                Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(ground_size)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.38, 0.65, 0.28), // grass green
                    perceptual_roughness: 0.95,
                    ..default()
                })),
                Transform::from_xyz(0.0, -0.3, 0.0), // well below other surfaces to prevent z-fighting
                TerrainEntity,
            ));
        }

        // Spawn rider avatar
        let start_elevation = points
            .first()
            .map(|p| p.elevation_m as f32 * terrain_config.elevation_scale)
            .unwrap_or(0.0);

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.3, 0.1),
                ..default()
            })),
            Transform::from_xyz(0.0, start_elevation + 1.5, 0.0),
            RiderAvatar,
        ));
    }
}

/// Spawn vegetation entities (trees and bushes).
fn spawn_vegetation(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    vegetation: &[sykla_world::vegetation::VegetationInstance],
) {
    let tree_trunk = meshes.add(Cylinder::new(0.3, 4.0));
    let tree_top = meshes.add(Sphere::new(2.0));
    let bush_mesh = meshes.add(Sphere::new(1.2));

    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.3, 0.15),
        ..default()
    });
    let leaf_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.6, 0.15),
        ..default()
    });
    let bush_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.55, 0.2),
        ..default()
    });

    for instance in vegetation {
        let [x, y, z] = instance.position;
        let s = instance.scale;

        match instance.kind {
            VegetationType::Tree => {
                commands.spawn((
                    Mesh3d(tree_trunk.clone()),
                    MeshMaterial3d(trunk_mat.clone()),
                    Transform::from_xyz(x, y + 2.0 * s, z).with_scale(Vec3::splat(s)),
                    TreeEntity,
                ));
                commands.spawn((
                    Mesh3d(tree_top.clone()),
                    MeshMaterial3d(leaf_mat.clone()),
                    Transform::from_xyz(x, y + 5.0 * s, z).with_scale(Vec3::splat(s)),
                    TreeEntity,
                ));
            }
            VegetationType::Bush => {
                commands.spawn((
                    Mesh3d(bush_mesh.clone()),
                    MeshMaterial3d(bush_mat.clone()),
                    Transform::from_xyz(x, y + 0.6 * s, z).with_scale(Vec3::splat(s)),
                    TreeEntity,
                ));
            }
        }
    }
}

/// Fill empty city space with trees — Waze-style fill-and-reject.
///
/// 1. Compute city bounds from road network
/// 2. Build a 2D occupancy grid marking roads, buildings, and water
/// 3. Spawn trees only in unoccupied cells
///
/// Trees can never appear on roads or inside buildings by construction.
fn spawn_city_vegetation(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    city: &CityData,
) {
    let road_network = match &city.road_network {
        Some(rn) => rn,
        None => return,
    };

    // --- 1. Compute city bounds from road nodes ---
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
    // Pad bounds so trees extend slightly beyond the road network
    let bound_pad = 40.0;
    min_x -= bound_pad;
    max_x += bound_pad;
    min_z -= bound_pad;
    max_z += bound_pad;

    // --- 2. Build occupancy grid ---
    let cell = 5.0_f32; // 5m per cell
    let cols = ((max_x - min_x) / cell).ceil() as usize + 1;
    let rows = ((max_z - min_z) / cell).ceil() as usize + 1;
    if cols * rows > 2_000_000 {
        return; // safety: don't allocate huge grids
    }
    let mut grid = vec![false; cols * rows];

    // Mark roads from centerlines (road surface + 3m sidewalk/buffer)
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

    // Mark buildings (1-cell padding keeps trees off walls)
    for mesh in &city.buildings {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 1);
    }
    // Mark road mesh triangles (belt-and-suspenders with centerline approach)
    for mesh in &city.roads {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 1);
    }
    // Mark water
    for mesh in &city.water {
        mark_mesh_triangles(&mut grid, mesh, min_x, min_z, cell, cols, rows, 0);
    }

    // --- 3. Spawn trees in empty cells ---
    let trunk_a = meshes.add(Cylinder::new(0.15, 4.5));
    let trunk_b = meshes.add(Cylinder::new(0.10, 5.5));
    let trunk_c = meshes.add(Cylinder::new(0.12, 3.0));
    let canopy_a = meshes.add(Sphere::new(1.3));
    let canopy_b = meshes.add(Sphere::new(0.8));
    let canopy_c = meshes.add(Sphere::new(1.0));

    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.10),
        perceptual_roughness: 0.95,
        ..default()
    });
    let greens: [Handle<StandardMaterial>; 4] = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.48, 0.10),
            perceptual_roughness: 0.85,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.55, 0.16),
            perceptual_roughness: 0.85,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.58, 0.20),
            perceptual_roughness: 0.80,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.52, 0.14),
            perceptual_roughness: 0.85,
            ..default()
        }),
    ];

    for r in 0..rows {
        for c in 0..cols {
            if grid[r * cols + c] {
                continue; // occupied — road, building, or water
            }

            let wx = min_x + (c as f32 + 0.5) * cell;
            let wz = min_z + (r as f32 + 0.5) * cell;
            let h = pos_hash(wx, wz);

            // ~25% of empty cells get a tree
            if h % 100 >= 25 {
                continue;
            }

            let tree_type = h % 3;
            let green_idx = ((h / 3) % 4) as usize;
            let scale = 0.80 + (h % 350) as f32 / 700.0; // 0.80 – 1.30

            // Jitter within cell (stay inside ±40% so trees don't drift into neighbors)
            let jx = ((h / 7) % 100) as f32 / 100.0 * cell * 0.8 - cell * 0.4;
            let jz = ((h / 11) % 100) as f32 / 100.0 * cell * 0.8 - cell * 0.4;
            let tx = wx + jx;
            let tz = wz + jz;

            let (trunk, trunk_y, canopy, canopy_y, cscale) = match tree_type {
                0 => (
                    // Round canopy (oak/maple)
                    &trunk_a,
                    2.25 * scale,
                    &canopy_a,
                    5.2 * scale,
                    Vec3::new(scale, scale * 0.85, scale),
                ),
                1 => (
                    // Tall narrow (crepe myrtle / poplar)
                    &trunk_b,
                    2.75 * scale,
                    &canopy_b,
                    6.2 * scale,
                    Vec3::new(scale * 0.65, scale * 1.5, scale * 0.65),
                ),
                _ => (
                    // Small ornamental
                    &trunk_c,
                    1.5 * scale,
                    &canopy_c,
                    3.8 * scale,
                    Vec3::new(scale * 0.9, scale * 0.9, scale * 0.9),
                ),
            };

            commands.spawn((
                Mesh3d(trunk.clone()),
                MeshMaterial3d(trunk_mat.clone()),
                Transform::from_xyz(tx, trunk_y, tz).with_scale(Vec3::splat(scale)),
                TreeEntity,
            ));
            commands.spawn((
                Mesh3d(canopy.clone()),
                MeshMaterial3d(greens[green_idx].clone()),
                Transform::from_xyz(tx, canopy_y, tz).with_scale(cscale),
                TreeEntity,
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Occupancy grid helpers
// ---------------------------------------------------------------------------

/// Mark a single cell in the occupancy grid.
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

/// Mark occupancy grid cells covered by a CityMesh's triangle bounding boxes (XZ projection).
///
/// For each triangle we fill its axis-aligned bounding box in the grid, plus optional
/// padding cells. This conservatively marks the footprint of buildings, roads, and water.
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

        // XZ bounding box of the triangle
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

/// Deterministic hash from 2D position — used for tree variety without randomness.
fn pos_hash(x: f32, z: f32) -> u32 {
    let a = (x * 73.856 + 19.143).to_bits();
    let b = (z * 37.292 + 47.857).to_bits();
    a.wrapping_mul(2654435761).wrapping_add(b.wrapping_mul(2246822519))
}

/// Spawn pre-batched city geometry (buildings, land use, water).
///
/// The .sykla file already contains meshes batched by FeatureSubtype (~10 total),
/// so we just render them directly — no grouping needed at runtime.
fn spawn_city_data(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    city: &CityData,
) {
    let all_meshes = city
        .buildings
        .iter()
        .chain(city.landuse.iter())
        .chain(city.water.iter())
        .chain(city.roads.iter());

    for city_mesh in all_meshes {
        if city_mesh.vertices.is_empty() {
            continue;
        }

        let color = city_mesh.color;
        let is_water = matches!(
            city_mesh.feature_subtype,
            FeatureSubtype::Lake | FeatureSubtype::Stream | FeatureSubtype::WaterOther
        );
        let is_road = matches!(
            city_mesh.feature_subtype,
            FeatureSubtype::Highway
                | FeatureSubtype::MainStreet
                | FeatureSubtype::Street
                | FeatureSubtype::Path
                | FeatureSubtype::RoadOther
        );
        let is_parking_lot = city_mesh.feature_subtype == FeatureSubtype::ParkingLot;
        let is_parked_car = city_mesh.feature_subtype == FeatureSubtype::ParkedCar;
        let is_lane_marking = matches!(
            city_mesh.feature_subtype,
            FeatureSubtype::CenterLine | FeatureSubtype::EdgeLine
        );
        let is_window_glass = city_mesh.feature_subtype == FeatureSubtype::WindowGlass;
        let is_window_frame = city_mesh.feature_subtype == FeatureSubtype::WindowFrame;
        let is_cornice = city_mesh.feature_subtype == FeatureSubtype::Cornice;
        let is_storefront = city_mesh.feature_subtype == FeatureSubtype::Storefront;

        let bevy_mesh = city_mesh_to_bevy(city_mesh);
        let material = if is_lane_marking {
            // Lane markings: unlit so they're bright and visible on dark asphalt
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                unlit: true,
                ..default()
            }
        } else if is_window_glass {
            // Window glass: unlit so it always reads as dark rectangles
            // PBR glass reflects the sky and becomes invisible against walls
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                unlit: true,
                ..default()
            }
        } else if is_storefront {
            // Storefront glass: unlit, dark plate glass
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                unlit: true,
                ..default()
            }
        } else if is_window_frame {
            // Window frame: matte painted wood/stone
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.9,
                ..default()
            }
        } else if is_cornice {
            // Cornice: matte stone
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.85,
                ..default()
            }
        } else if is_parked_car {
            // Car paint: slightly glossy
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.4,
                metallic: 0.1,
                ..default()
            }
        } else if is_parking_lot {
            // Parking lot asphalt: similar to road
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.7,
                ..default()
            }
        } else if is_water {
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.1,
                metallic: 0.3,
                ..default()
            }
        } else if is_road {
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.7,
                ..default()
            }
        } else {
            // Buildings and landuse: PBR with matte finish
            StandardMaterial {
                base_color: Color::srgba(color[0], color[1], color[2], color[3]),
                perceptual_roughness: 0.85,
                ..default()
            }
        };

        commands.spawn((
            Mesh3d(meshes.add(bevy_mesh)),
            MeshMaterial3d(materials.add(material)),
            CityEntity,
        ));
    }
}

/// Update camera and rider position to follow along the route.
fn update_camera_system(
    ride: Option<Res<ActiveRide>>,
    mut rider_query: Query<&mut Transform, (With<RiderAvatar>, Without<Camera3d>)>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    let Some(ride) = ride else { return };

    let state = ride.engine.state();
    let distance = state.distance_m as f32;
    let elevation = state.current_elevation_m as f32;

    // Update rider position (route-relative: X=0, Y=elevation, Z=distance)
    for mut transform in rider_query.iter_mut() {
        transform.translation = Vec3::new(0.0, elevation + 1.5, distance);
    }

    // Camera follows behind and above the rider
    for mut transform in camera_query.iter_mut() {
        let target = Vec3::new(0.0, elevation + 1.5, distance);
        let camera_pos = Vec3::new(0.0, elevation + 15.0, distance - 25.0);
        transform.translation = transform.translation.lerp(camera_pos, 0.05);
        *transform = transform.looking_at(target, Vec3::Y);
    }
}

/// Convert a CityMesh to a Bevy Mesh (no UVs needed — flat color).
fn city_mesh_to_bevy(city_mesh: &CityMesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::default(),
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, city_mesh.vertices.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, city_mesh.normals.clone());
    // Generate dummy UVs (required by StandardMaterial)
    let uvs: Vec<[f32; 2]> = vec![[0.0, 0.0]; city_mesh.vertices.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(city_mesh.indices.clone()));

    mesh
}

/// Convert world generation data into a Bevy Mesh.
fn mesh_from_world_data(
    vertices: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::default(),
    );

    let positions: Vec<[f32; 3]> = vertices.to_vec();
    let norms: Vec<[f32; 3]> = normals.to_vec();
    let tex_coords: Vec<[f32; 2]> = uvs.to_vec();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norms);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tex_coords);
    mesh.insert_indices(Indices::U32(indices.to_vec()));

    mesh
}
