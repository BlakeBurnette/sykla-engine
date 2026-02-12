use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::ble::TrainerPlugin;
use crate::city_loader::CityLoaderPlugin;
use crate::free_roam::FreeRoamPlugin;
use crate::hud::HudPlugin;
use crate::ride::RidePlugin;
use crate::sync_client::SyncPlugin;
use crate::world_render::WorldRenderPlugin;

/// Application states for the Sykla cycling simulator.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Menu,
    RouteSelect,
    Riding,
    Summary,
}

/// Top-level Sykla application.
pub struct SyklaApp;

impl SyklaApp {
    pub fn run() {
        App::new()
            .add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Sykla — Ride the World".into(),
                    canvas: Some("#sykla-canvas".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            }))
            // Hazy blue horizon — matches fog color for seamless sky/horizon blend
            .insert_resource(ClearColor(Color::srgb(0.75, 0.85, 0.95)))
            .init_state::<AppState>()
            .add_plugins((
                TrainerPlugin,
                WorldRenderPlugin,
                RidePlugin,
                HudPlugin,
                SyncPlugin,
                CityLoaderPlugin,
                FreeRoamPlugin,
            ))
            .add_systems(Startup, setup_camera)
            .run();
    }
}

fn setup_camera(mut commands: Commands) {
    // 3D camera — AcesFitted tonemapping (no LUT, WebGL2 compatible, preserves color)
    // Start at origin, ~30m up looking slightly down
    commands.spawn((
        Camera3d::default(),
        Tonemapping::AcesFitted,
        Transform::from_xyz(0.0, 30.0, 0.0).looking_at(Vec3::new(0.0, 0.0, 50.0), Vec3::Y),
        // Distance fog — fades geometry into hazy blue horizon
        DistanceFog {
            color: Color::srgb(0.75, 0.85, 0.95),
            directional_light_color: Color::srgb(1.0, 0.95, 0.85),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::Linear {
                start: 300.0,
                end: 1500.0,
            },
        },
    ));

    // Sun — strong directional light for visible shading contrast on building walls
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 0.97, 0.90), // warm sunlight
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    ));

    // Ambient light — reduced fill for stronger sun-to-ambient contrast
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.85, 0.88, 0.95), // cool sky fill
        brightness: 350.0,
    });
}

/// Entry point called from WASM.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    SyklaApp::run();
}
