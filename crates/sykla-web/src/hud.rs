use bevy::prelude::*;

use crate::ble::TrainerState;
use crate::free_roam::{turn_label, FreeRoamState, NavMode};
use crate::ride::ActiveRide;
use crate::world_render::LoadedRoadNetwork;

/// Marker component for the HUD root node.
#[derive(Component)]
pub struct HudRoot;

/// Marker for the speed text.
#[derive(Component)]
pub struct SpeedText;

/// Marker for the power text.
#[derive(Component)]
pub struct PowerText;

/// Marker for the cadence text.
#[derive(Component)]
pub struct CadenceText;

/// Marker for the grade text.
#[derive(Component)]
pub struct GradeText;

/// Marker for the distance text.
#[derive(Component)]
pub struct DistanceText;

/// Marker for the elapsed time text.
#[derive(Component)]
pub struct TimeText;

/// Marker for the intersection turn overlay.
#[derive(Component)]
pub struct TurnOverlay;

/// Marker for the mode indicator text.
#[derive(Component)]
pub struct ModeText;

/// Marker for the street name display.
#[derive(Component)]
pub struct StreetNameText;

/// Plugin for the 2D HUD overlay.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, (update_hud, update_turn_overlay, update_mode_text, update_street_name));
    }
}

fn setup_hud(mut commands: Commands) {
    let text_font = TextFont {
        font_size: 24.0,
        ..default()
    };

    let bold_font = TextFont {
        font_size: 32.0,
        ..default()
    };

    // Root node for the HUD — bottom-left panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                bottom: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            HudRoot,
        ))
        .with_children(|parent| {
            // Speed
            parent.spawn((
                Text::new("0.0 km/h"),
                bold_font.clone(),
                TextColor(Color::WHITE),
                SpeedText,
            ));
            // Power
            parent.spawn((
                Text::new("0 W"),
                text_font.clone(),
                TextColor(Color::srgb(1.0, 0.8, 0.2)),
                PowerText,
            ));
            // Cadence
            parent.spawn((
                Text::new("0 rpm"),
                text_font.clone(),
                TextColor(Color::srgb(0.5, 0.8, 1.0)),
                CadenceText,
            ));
            // Grade
            parent.spawn((
                Text::new("0.0%"),
                text_font.clone(),
                TextColor(Color::srgb(0.8, 1.0, 0.8)),
                GradeText,
            ));
            // Distance
            parent.spawn((
                Text::new("0.00 km"),
                text_font.clone(),
                TextColor(Color::WHITE),
                DistanceText,
            ));
            // Time
            parent.spawn((
                Text::new("00:00"),
                text_font.clone(),
                TextColor(Color::WHITE),
                TimeText,
            ));
        });

    // Street name — bottom-center
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Percent(50.0),
            padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
    )).with_children(|parent| {
        parent.spawn((
            Text::new(""),
            TextFont {
                font_size: 26.0,
                ..default()
            },
            TextColor(Color::WHITE),
            StreetNameText,
        ));
    });

    // Turn overlay — top-center, hidden by default
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Percent(50.0),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        TurnOverlay,
    )).with_children(|parent| {
        parent.spawn((
            Text::new(""),
            TextFont {
                font_size: 28.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });

    // Mode indicator — top-right corner
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            right: Val::Px(20.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Road Mode"),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 1.0, 0.7)),
            ModeText,
        ));
    });
}

fn update_hud(
    trainer: Res<TrainerState>,
    ride: Option<Res<ActiveRide>>,
    mut speed_q: Query<&mut Text, (With<SpeedText>, Without<PowerText>, Without<CadenceText>, Without<GradeText>, Without<DistanceText>, Without<TimeText>)>,
    mut power_q: Query<&mut Text, (With<PowerText>, Without<SpeedText>, Without<CadenceText>, Without<GradeText>, Without<DistanceText>, Without<TimeText>)>,
    mut cadence_q: Query<&mut Text, (With<CadenceText>, Without<SpeedText>, Without<PowerText>, Without<GradeText>, Without<DistanceText>, Without<TimeText>)>,
    mut grade_q: Query<&mut Text, (With<GradeText>, Without<SpeedText>, Without<PowerText>, Without<CadenceText>, Without<DistanceText>, Without<TimeText>)>,
    mut distance_q: Query<&mut Text, (With<DistanceText>, Without<SpeedText>, Without<PowerText>, Without<CadenceText>, Without<GradeText>, Without<TimeText>)>,
    mut time_q: Query<&mut Text, (With<TimeText>, Without<SpeedText>, Without<PowerText>, Without<CadenceText>, Without<GradeText>, Without<DistanceText>)>,
) {
    let state = ride.as_ref().map(|r| r.engine.state());

    for mut text in speed_q.iter_mut() {
        **text = format!("{:.1} km/h", trainer.speed_kmh);
    }
    for mut text in power_q.iter_mut() {
        **text = format!("{:.0} W", trainer.power_watts);
    }
    for mut text in cadence_q.iter_mut() {
        **text = format!("{:.0} rpm", trainer.cadence_rpm);
    }

    if let Some(state) = state {
        for mut text in grade_q.iter_mut() {
            let arrow = if state.current_grade_percent > 0.5 {
                "^"
            } else if state.current_grade_percent < -0.5 {
                "v"
            } else {
                "-"
            };
            **text = format!("{}{:.1}%", arrow, state.current_grade_percent);
        }
        for mut text in distance_q.iter_mut() {
            **text = format!("{:.2} km", state.distance_m / 1000.0);
        }
        for mut text in time_q.iter_mut() {
            let secs = state.elapsed_secs as u64;
            let mins = secs / 60;
            let remaining = secs % 60;
            **text = format!("{mins:02}:{remaining:02}");
        }
    }
}

/// Update the current street name display.
fn update_street_name(
    nav: Res<FreeRoamState>,
    road_net: Option<Res<LoadedRoadNetwork>>,
    mut name_q: Query<&mut Text, With<StreetNameText>>,
    mut parent_q: Query<&mut BackgroundColor, With<Children>>,
) {
    let street_name = if nav.mode == NavMode::Road && nav.initialized {
        if let Some(ref rn) = road_net {
            let edge_idx = nav.current_edge as usize;
            if edge_idx < rn.network.edges.len() {
                rn.network.edges[edge_idx].name.clone()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    for mut text in name_q.iter_mut() {
        if let Some(ref name) = street_name {
            **text = name.clone();
        } else {
            **text = String::new();
        }
    }

    // Show/hide background for the street name container
    // We need to find the parent of StreetNameText — use a simpler approach:
    // just make the text empty when no name, which effectively hides it
    let _ = parent_q; // unused — background stays transparent, text visibility is enough
}

/// Update the intersection turn overlay.
fn update_turn_overlay(
    nav: Res<FreeRoamState>,
    road_net: Option<Res<LoadedRoadNetwork>>,
    mut overlay_q: Query<(&mut BackgroundColor, &Children), With<TurnOverlay>>,
    mut text_q: Query<&mut Text, (Without<TurnOverlay>, Without<StreetNameText>, Without<ModeText>, Without<SpeedText>)>,
) {
    let show = nav.mode == NavMode::Road && nav.at_intersection && !nav.turn_choices.is_empty();

    for (mut bg, children) in overlay_q.iter_mut() {
        if show {
            *bg = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75));

            if let Some(ref rn) = road_net {
                let network = &rn.network;
                let edge = &network.edges[nav.current_edge as usize];
                let node_idx = if nav.progress >= 0.5 {
                    edge.node_b
                } else {
                    edge.node_a
                };

                let mut parts: Vec<String> = Vec::new();
                for (i, &eidx) in nav.turn_choices.iter().enumerate() {
                    let direction = turn_label(&nav, network, eidx, node_idx);
                    let edge_name = network.edges[eidx as usize]
                        .name
                        .as_deref()
                        .unwrap_or("unnamed");
                    let entry = format!("{} ({})", direction, edge_name);
                    if i == nav.selected_turn {
                        parts.push(format!("[> {} <]", entry));
                    } else {
                        parts.push(format!("  {}  ", entry));
                    }
                }

                let display = parts.join(" | ");

                for &child in children.iter() {
                    if let Ok(mut text) = text_q.get_mut(child) {
                        **text = display.clone();
                    }
                }
            }
        } else {
            *bg = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0));
            for &child in children.iter() {
                if let Ok(mut text) = text_q.get_mut(child) {
                    **text = String::new();
                }
            }
        }
    }
}

/// Update the mode indicator text.
fn update_mode_text(
    nav: Res<FreeRoamState>,
    mut mode_q: Query<(&mut Text, &mut TextColor), With<ModeText>>,
) {
    for (mut text, mut color) in mode_q.iter_mut() {
        match nav.mode {
            NavMode::Road => {
                **text = "Road Mode [Tab]".to_string();
                *color = TextColor(Color::srgb(0.7, 1.0, 0.7));
            }
            NavMode::FreeRoam => {
                **text = "Free Roam [Tab]".to_string();
                *color = TextColor(Color::srgb(1.0, 0.8, 0.5));
            }
        }
    }
}
