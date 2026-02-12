use bevy::prelude::*;

use crate::ride::ActiveRide;
use crate::world_render::LoadedRoadNetwork;

/// Navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    /// Constrained to roads, auto-continues at intersections.
    Road,
    /// Free-flying camera (original behavior).
    FreeRoam,
}

/// Turn preference detected from held keys (A/D) at intersections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnPreference {
    Straight,
    Left,
    Right,
}

/// Resource tracking navigation state (road-constrained + free-roam).
#[derive(Resource)]
pub struct FreeRoamState {
    pub mode: NavMode,
    /// Current edge index in the road network.
    pub current_edge: u32,
    /// Progress along the current edge (0.0 = node_a, 1.0 = node_b).
    pub progress: f32,
    /// Whether traveling from node_a to node_b (true) or reversed.
    pub forward: bool,
    /// Movement speed in m/s (default 15 ≈ 34 mph).
    pub speed: f32,
    /// Whether we're stopped at an intersection.
    pub at_intersection: bool,
    /// Available edge indices at the current intersection.
    pub turn_choices: Vec<u32>,
    /// Index into turn_choices of the currently highlighted option.
    pub selected_turn: usize,
    /// Smoothed camera yaw (radians).
    pub camera_yaw: f32,
    /// Whether road navigation has been initialized.
    pub initialized: bool,

    // --- FreeRoam-only state ---
    pub free_yaw: f32,
    pub free_pitch: f32,
}

impl Default for FreeRoamState {
    fn default() -> Self {
        Self {
            mode: NavMode::Road,
            current_edge: 0,
            progress: 0.0,
            forward: true,
            speed: 15.0,
            at_intersection: false,
            turn_choices: Vec::new(),
            selected_turn: 0,
            camera_yaw: 0.0,
            initialized: false,
            free_yaw: 0.0,
            free_pitch: -0.3,
        }
    }
}

/// Plugin for dual-mode navigation (road-constrained + free-roam).
pub struct FreeRoamPlugin;

impl Plugin for FreeRoamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FreeRoamState>()
            .add_systems(Update, navigation_system);
    }
}

/// Main navigation system — dispatches to road or free-roam mode.
fn navigation_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    ride: Option<Res<ActiveRide>>,
    road_net: Option<Res<LoadedRoadNetwork>>,
    mut state: ResMut<FreeRoamState>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    // Only active when there's no ride
    if ride.is_some() {
        return;
    }

    let dt = time.delta_secs();
    if dt <= 0.0 || dt > 0.5 {
        return;
    }

    // Toggle mode with Tab
    if keys.just_pressed(KeyCode::Tab) {
        state.mode = match state.mode {
            NavMode::Road => {
                // Save current camera transform into free-roam state
                if let Ok(transform) = camera_query.get_single() {
                    let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
                    state.free_yaw = yaw;
                    state.free_pitch = pitch;
                }
                NavMode::FreeRoam
            }
            NavMode::FreeRoam => {
                // Snap back to road
                state.at_intersection = false;
                NavMode::Road
            }
        };
    }

    // Speed adjustment (both modes)
    if keys.just_pressed(KeyCode::KeyR) {
        state.speed = (state.speed * 1.5).min(200.0);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        state.speed = (state.speed / 1.5).max(3.0);
    }

    match state.mode {
        NavMode::Road => {
            if let Some(ref rn) = road_net {
                road_mode_update(&mut state, &rn.network, &keys, dt, &mut camera_query);
            } else {
                // No road network — fall back to free-roam
                free_roam_update(&mut state, &keys, dt, &mut camera_query);
            }
        }
        NavMode::FreeRoam => {
            free_roam_update(&mut state, &keys, dt, &mut camera_query);
        }
    }
}

/// Road-constrained navigation.
fn road_mode_update(
    state: &mut ResMut<FreeRoamState>,
    network: &sykla_world::road_network::RoadNetwork,
    keys: &Res<ButtonInput<KeyCode>>,
    dt: f32,
    camera_query: &mut Query<&mut Transform, With<Camera3d>>,
) {
    if network.edges.is_empty() {
        return;
    }

    // Initialize: snap to nearest road to city center
    if !state.initialized {
        if let Some((edge_idx, progress, _dist_sq)) = network.nearest_edge(0.0, 0.0) {
            state.current_edge = edge_idx;
            state.progress = progress;
            state.forward = true;
            state.initialized = true;
        }
    }

    let edge_idx = state.current_edge as usize;
    if edge_idx >= network.edges.len() {
        return;
    }

    let sprint = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        3.0
    } else {
        1.0
    };

    let edge = &network.edges[edge_idx];
    if edge.length_m > 1e-6 {
        let move_frac = state.speed * sprint * dt / edge.length_m;

        // S / Down = U-turn
        if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
            state.forward = !state.forward;
        }

        // W / Up = pedal forward (stop when not pressing)
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            if state.forward {
                state.progress += move_frac;
            } else {
                state.progress -= move_frac;
            }
        }

        // Detect turn preference from held A/D keys
        let turn_pref = if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            TurnPreference::Left
        } else if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            TurnPreference::Right
        } else {
            TurnPreference::Straight
        };

        // Check if we've reached a node — auto-continue, never stop
        if state.progress >= 1.0 {
            state.progress = 1.0;
            arrive_at_node(state, network, edge.node_b, turn_pref);
        } else if state.progress <= 0.0 {
            state.progress = 0.0;
            arrive_at_node(state, network, edge.node_a, turn_pref);
        }
    }

    // Camera: sit on the road, look at a point 3m ahead. No angle math.
    let edge = &network.edges[state.current_edge as usize];
    let pos = edge.sample_position(state.progress);

    let ahead_frac = (3.0 / edge.length_m).clamp(0.005, 0.15);
    let ahead_progress = if state.forward {
        (state.progress + ahead_frac).min(1.0)
    } else {
        (state.progress - ahead_frac).max(0.0)
    };
    let ahead = edge.sample_position(ahead_progress);

    let eye = Vec3::new(pos[0], 3.0, pos[1]);
    let target = Vec3::new(ahead[0], 2.8, ahead[1]); // slight downward look

    for mut transform in camera_query.iter_mut() {
        transform.translation = eye;
        if (target - eye).length_squared() > 0.001 {
            transform.look_at(target, Vec3::Y);
        }
        // Store yaw for mode-switch consistency
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        state.camera_yaw = yaw;
    }
}

/// Called when the player reaches a node. Always auto-continues — never stops.
/// Hold A/D to steer left/right at intersections; otherwise takes the straightest path.
fn arrive_at_node(
    state: &mut ResMut<FreeRoamState>,
    network: &sykla_world::road_network::RoadNetwork,
    node_idx: u32,
    turn_pref: TurnPreference,
) {
    let node = &network.nodes[node_idx as usize];
    let current_edge = state.current_edge;

    // Edges connected to this node, excluding the one we came from
    let other_edges: Vec<u32> = node
        .edge_ids
        .iter()
        .copied()
        .filter(|&e| e != current_edge)
        .collect();

    if other_edges.is_empty() {
        // Dead end — auto U-turn
        state.forward = !state.forward;
        return;
    }

    if other_edges.len() == 1 {
        // Only one way to go — take it
        transition_to_edge(state, network, other_edges[0], node_idx);
        return;
    }

    // Multiple choices — always auto-continue, never stop
    let on_roundabout = network.edges[current_edge as usize].is_roundabout;
    let roundabout_edges: Vec<u32> = other_edges
        .iter()
        .copied()
        .filter(|&e| network.edges[e as usize].is_roundabout)
        .collect();
    let normal_edges: Vec<u32> = other_edges
        .iter()
        .copied()
        .filter(|&e| !network.edges[e as usize].is_roundabout)
        .collect();

    let choice = if on_roundabout {
        // On a roundabout: A/D = exit, otherwise continue around
        match turn_pref {
            TurnPreference::Straight => {
                // Stay on roundabout
                if !roundabout_edges.is_empty() {
                    let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
                    let best = find_straightest_turn(state, network, &sorted, node_idx);
                    sorted[best]
                } else {
                    pick_straightest(state, network, &other_edges, node_idx)
                }
            }
            TurnPreference::Left | TurnPreference::Right => {
                // Exit roundabout if exits are available
                if !normal_edges.is_empty() {
                    let sorted = sort_turns_by_angle(state, network, &normal_edges, node_idx);
                    match turn_pref {
                        TurnPreference::Left => sorted[0],
                        _ => *sorted.last().unwrap(),
                    }
                } else if !roundabout_edges.is_empty() {
                    // No exits — continue on roundabout
                    let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
                    let best = find_straightest_turn(state, network, &sorted, node_idx);
                    sorted[best]
                } else {
                    pick_straightest(state, network, &other_edges, node_idx)
                }
            }
        }
    } else if !roundabout_edges.is_empty() {
        // Entering a roundabout — auto-enter
        let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
        let best = find_straightest_turn(state, network, &sorted, node_idx);
        sorted[best]
    } else {
        // Normal intersection
        match turn_pref {
            TurnPreference::Left => {
                let sorted = sort_turns_by_angle(state, network, &other_edges, node_idx);
                sorted[0] // leftmost
            }
            TurnPreference::Right => {
                let sorted = sort_turns_by_angle(state, network, &other_edges, node_idx);
                *sorted.last().unwrap() // rightmost
            }
            TurnPreference::Straight => {
                pick_straightest(state, network, &other_edges, node_idx)
            }
        }
    };

    transition_to_edge(state, network, choice, node_idx);
}

/// Pick the straightest continuation: same-name priority, then angle-based.
fn pick_straightest(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
    edges: &[u32],
    node_idx: u32,
) -> u32 {
    // Same street name gets priority (handles curves)
    let current_name = network.edges[state.current_edge as usize].name.as_deref();
    if let Some(name) = current_name {
        let same_name: Vec<u32> = edges
            .iter()
            .copied()
            .filter(|&e| network.edges[e as usize].name.as_deref() == Some(name))
            .collect();
        if same_name.len() == 1 {
            return same_name[0];
        }
    }
    // Fall back to smallest angle deviation
    let sorted = sort_turns_by_angle(state, network, edges, node_idx);
    let best = find_straightest_turn(state, network, &sorted, node_idx);
    sorted[best]
}

/// Transition to a new edge from a given node.
fn transition_to_edge(
    state: &mut ResMut<FreeRoamState>,
    network: &sykla_world::road_network::RoadNetwork,
    next_edge_idx: u32,
    from_node: u32,
) {
    let next_edge = &network.edges[next_edge_idx as usize];

    if next_edge.node_a == from_node {
        state.forward = true;
        state.progress = 0.0;
    } else {
        state.forward = false;
        state.progress = 1.0;
    }

    state.current_edge = next_edge_idx;
    state.at_intersection = false;
    state.turn_choices.clear();
    state.selected_turn = 0;
}

/// Get the node index at the end we're approaching (or at, if stopped).
fn current_node_at_end(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
) -> u32 {
    let edge = &network.edges[state.current_edge as usize];
    if state.progress >= 0.5 {
        edge.node_b
    } else {
        edge.node_a
    }
}

/// Sort turn choices by angle relative to current travel direction.
/// Returns edges sorted left-to-right.
fn sort_turns_by_angle(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
    edges: &[u32],
    node_idx: u32,
) -> Vec<u32> {
    let current_edge = &network.edges[state.current_edge as usize];
    let inc = incoming_direction(current_edge, state.forward);
    let incoming_angle = inc[1].atan2(inc[0]);

    let mut edge_angles: Vec<(u32, f32)> = edges
        .iter()
        .map(|&eidx| {
            let edge = &network.edges[eidx as usize];
            let dir = leaving_direction(edge, node_idx);
            let angle = dir[1].atan2(dir[0]);
            let relative = angle_diff(incoming_angle, angle);
            (eidx, relative)
        })
        .collect();

    // Sort by relative angle (left = positive, right = negative)
    edge_angles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    edge_angles.into_iter().map(|(idx, _)| idx).collect()
}

/// Find the turn choice closest to straight ahead.
fn find_straightest_turn(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
    choices: &[u32],
    node_idx: u32,
) -> usize {
    if choices.is_empty() {
        return 0;
    }

    let current_edge = &network.edges[state.current_edge as usize];
    let inc = incoming_direction(current_edge, state.forward);
    let incoming_angle = inc[1].atan2(inc[0]);

    let mut best = 0;
    let mut best_abs = f32::MAX;

    for (i, &eidx) in choices.iter().enumerate() {
        let edge = &network.edges[eidx as usize];
        let dir = leaving_direction(edge, node_idx);
        let angle = dir[1].atan2(dir[0]);
        let relative = angle_diff(incoming_angle, angle).abs();
        if relative < best_abs {
            best_abs = relative;
            best = i;
        }
    }

    best
}

/// Get the signed turn angle in degrees for a given edge from a node.
fn turn_angle_degrees(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
    edge_idx: u32,
    node_idx: u32,
) -> f32 {
    let current_edge = &network.edges[state.current_edge as usize];
    let inc = incoming_direction(current_edge, state.forward);
    let incoming_angle = inc[1].atan2(inc[0]);

    let edge = &network.edges[edge_idx as usize];
    let dir = leaving_direction(edge, node_idx);
    let angle = dir[1].atan2(dir[0]);
    angle_diff(incoming_angle, angle).to_degrees()
}

/// Compute the turn label for a given edge from a node, relative to incoming direction.
pub fn turn_label(
    state: &FreeRoamState,
    network: &sykla_world::road_network::RoadNetwork,
    edge_idx: u32,
    node_idx: u32,
) -> &'static str {
    let current_edge = &network.edges[state.current_edge as usize];
    let inc = incoming_direction(current_edge, state.forward);
    let incoming_angle = inc[1].atan2(inc[0]);

    let edge = &network.edges[edge_idx as usize];
    let dir = leaving_direction(edge, node_idx);
    let angle = dir[1].atan2(dir[0]);
    let relative = angle_diff(incoming_angle, angle);

    let abs_deg = relative.to_degrees().abs();
    if abs_deg < 30.0 {
        "Straight"
    } else if abs_deg > 150.0 {
        "U-turn"
    } else if relative > 0.0 {
        "Left"
    } else {
        "Right"
    }
}

/// Stable incoming direction: the direction the rider was traveling when arriving at a node.
/// Samples a few meters before the endpoint to avoid noisy short terminal segments.
fn incoming_direction(
    edge: &sykla_world::road_network::RoadEdge,
    forward: bool,
) -> [f32; 2] {
    let frac = (5.0 / edge.length_m).clamp(0.01, 0.3);
    if forward {
        // Arriving at node_b — sample slightly before the end
        let d = edge.sample_direction((1.0 - frac).max(0.0));
        [d[0], d[1]]
    } else {
        // Arriving at node_a — sample slightly after the start
        let d = edge.sample_direction(frac.min(1.0));
        [-d[0], -d[1]]
    }
}

/// Stable direction leaving a node along a candidate edge.
/// Samples a few meters into the edge to avoid noisy short segments at the node.
fn leaving_direction(
    edge: &sykla_world::road_network::RoadEdge,
    node_idx: u32,
) -> [f32; 2] {
    let frac = (5.0 / edge.length_m).clamp(0.01, 0.3);
    if edge.node_a == node_idx {
        edge.sample_direction(frac)
    } else {
        let d = edge.sample_direction(1.0 - frac);
        [-d[0], -d[1]]
    }
}

/// Shortest signed angle difference (a → b), result in [-PI, PI].
fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = b - a;
    while d > std::f32::consts::PI {
        d -= 2.0 * std::f32::consts::PI;
    }
    while d < -std::f32::consts::PI {
        d += 2.0 * std::f32::consts::PI;
    }
    d
}

/// Free-roam camera (original behavior).
fn free_roam_update(
    state: &mut ResMut<FreeRoamState>,
    keys: &Res<ButtonInput<KeyCode>>,
    dt: f32,
    camera_query: &mut Query<&mut Transform, With<Camera3d>>,
) {
    let sprint = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        3.0
    } else {
        1.0
    };
    let speed = state.speed * sprint;

    // Turn (yaw)
    let turn_speed = 2.0;
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        state.free_yaw += turn_speed * dt;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        state.free_yaw -= turn_speed * dt;
    }

    // Pitch
    let pitch_speed = 1.5;
    if keys.pressed(KeyCode::KeyT) {
        state.free_pitch = (state.free_pitch + pitch_speed * dt).min(1.2);
    }
    if keys.pressed(KeyCode::KeyG) {
        state.free_pitch = (state.free_pitch - pitch_speed * dt).max(-1.2);
    }

    let forward = Vec3::new(-state.free_yaw.sin(), 0.0, state.free_yaw.cos());

    let mut velocity = Vec3::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        velocity += forward;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        velocity -= forward;
    }
    if keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::KeyQ) {
        velocity += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyE) {
        velocity -= Vec3::Y;
    }

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize() * speed * dt;
    }

    for mut transform in camera_query.iter_mut() {
        transform.translation += velocity;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, state.free_yaw, state.free_pitch, 0.0);
    }
}
