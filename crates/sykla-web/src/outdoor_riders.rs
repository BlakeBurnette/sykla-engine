use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world_render::WorldProjection;

/// Marker component for outdoor rider avatars.
#[derive(Component)]
pub struct OutdoorRiderAvatar {
    pub user_id: String,
}

/// Resource holding outdoor rider positions from the global WebSocket.
#[derive(Resource, Default)]
pub struct OutdoorRidersState {
    pub connected: bool,
    pub riders: Vec<OutdoorRiderData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutdoorRiderData {
    pub user_id: String,
    pub display_name: String,
    pub lat: f64,
    pub lng: f64,
    pub speed_kmh: f64,
    pub heading: f64,
    pub is_indoor: bool,
}

/// Plugin for rendering outdoor riders in the 3D world.
pub struct OutdoorRidersPlugin;

impl Plugin for OutdoorRidersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OutdoorRidersState>()
            .add_systems(Update, update_outdoor_riders);
    }
}

/// System that spawns/updates/despawns outdoor rider avatars based on
/// the global WebSocket feed.
fn update_outdoor_riders(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<OutdoorRidersState>,
    projection: Option<Res<WorldProjection>>,
    existing: Query<(Entity, &OutdoorRiderAvatar)>,
) {
    let Some(proj) = projection else { return };

    // Collect current avatar user_ids
    let mut existing_ids: std::collections::HashMap<String, Entity> = existing
        .iter()
        .map(|(e, a)| (a.user_id.clone(), e))
        .collect();

    for rider in &state.riders {
        let (x, z) = proj.projection.project(rider.lat, rider.lng);

        if let Some(&entity) = existing_ids.get(&rider.user_id) {
            // Avatar already exists — position will be updated by the
            // update_outdoor_rider_positions system. Just mark as still active.
            existing_ids.remove(&rider.user_id);
            let _ = entity;
        } else {
            // Spawn new outdoor rider avatar (green sphere to distinguish from indoor orange)
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.8, 0.3), // green for outdoor
                    ..default()
                })),
                Transform::from_xyz(x as f32, 1.5, z as f32),
                OutdoorRiderAvatar {
                    user_id: rider.user_id.clone(),
                },
            ));
        }
    }

    // Despawn avatars for riders no longer present
    for (_id, entity) in existing_ids {
        commands.entity(entity).despawn_recursive();
    }
}
