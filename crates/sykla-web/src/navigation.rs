use sykla_engine::glam::Vec3;
use sykla_world::road_network::RoadNetwork;
use winit::keyboard::KeyCode;

/// Navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Road,
    FreeRoam,
}

/// Persistent key state — maintained by winit event handler.
#[derive(Default)]
pub struct KeyState {
    pressed: std::collections::HashSet<KeyCode>,
    just_pressed: std::collections::HashSet<KeyCode>,
}

impl KeyState {
    pub fn update(&mut self, code: KeyCode, pressed: bool) {
        if pressed {
            if self.pressed.insert(code) {
                self.just_pressed.insert(code);
            }
        } else {
            self.pressed.remove(&code);
        }
    }

    pub fn pressed(&self, code: KeyCode) -> bool {
        self.pressed.contains(&code)
    }

    pub fn just_pressed(&self, code: KeyCode) -> bool {
        self.just_pressed.contains(&code)
    }

    /// Clear just_pressed state. Call at the end of each frame.
    pub fn end_frame(&mut self) {
        self.just_pressed.clear();
    }
}

/// Turn preference detected from held keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnPreference {
    Straight,
    Left,
    Right,
}

/// Navigation state — plain struct, no Bevy.
pub struct NavigationState {
    pub mode: NavMode,
    pub current_edge: u32,
    pub progress: f32,
    pub forward: bool,
    pub speed: f32,
    pub at_intersection: bool,
    pub turn_choices: Vec<u32>,
    pub selected_turn: usize,
    pub camera_yaw: f32,
    pub initialized: bool,
    pub free_yaw: f32,
    pub free_pitch: f32,
    // Free-roam position tracking
    free_pos: Vec3,
}

impl Default for NavigationState {
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
            free_pos: Vec3::new(0.0, 30.0, 0.0),
        }
    }
}

impl NavigationState {
    /// Update navigation state and return (eye, target) for camera.
    pub fn update(
        &mut self,
        dt: f32,
        keys: &KeyState,
        network: Option<&RoadNetwork>,
    ) -> (Vec3, Vec3) {
        if dt <= 0.0 || dt > 0.5 {
            return (self.free_pos, self.free_pos + Vec3::Z);
        }

        // Toggle mode with Tab
        if keys.just_pressed(KeyCode::Tab) {
            self.mode = match self.mode {
                NavMode::Road => NavMode::FreeRoam,
                NavMode::FreeRoam => {
                    self.at_intersection = false;
                    NavMode::Road
                }
            };
        }

        // Speed adjustment (both modes)
        if keys.just_pressed(KeyCode::KeyR) {
            self.speed = (self.speed * 1.5).min(200.0);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            self.speed = (self.speed / 1.5).max(3.0);
        }

        match self.mode {
            NavMode::Road => {
                if let Some(network) = network {
                    self.road_mode_update(network, keys, dt)
                } else {
                    self.free_roam_update(keys, dt)
                }
            }
            NavMode::FreeRoam => self.free_roam_update(keys, dt),
        }
    }

    pub fn current_street_name<'a>(&self, network: Option<&'a RoadNetwork>) -> Option<&'a str> {
        if self.mode != NavMode::Road || !self.initialized {
            return None;
        }
        let network = network?;
        let edge_idx = self.current_edge as usize;
        if edge_idx >= network.edges.len() {
            return None;
        }
        network.edges[edge_idx].name.as_deref()
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            NavMode::Road => "Road Mode [Tab]",
            NavMode::FreeRoam => "Free Roam [Tab]",
        }
    }

    fn road_mode_update(
        &mut self,
        network: &RoadNetwork,
        keys: &KeyState,
        dt: f32,
    ) -> (Vec3, Vec3) {
        if network.edges.is_empty() {
            return self.free_roam_update(keys, dt);
        }

        // Initialize: snap to nearest road to city center
        if !self.initialized {
            if let Some((edge_idx, progress, _dist_sq)) = network.nearest_edge(0.0, 0.0) {
                self.current_edge = edge_idx;
                self.progress = progress;
                self.forward = true;
                self.initialized = true;
            }
        }

        let edge_idx = self.current_edge as usize;
        if edge_idx >= network.edges.len() {
            return self.free_roam_update(keys, dt);
        }

        let sprint = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            3.0
        } else {
            1.0
        };

        let edge = &network.edges[edge_idx];
        if edge.length_m > 1e-6 {
            let move_frac = self.speed * sprint * dt / edge.length_m;

            // S / Down = U-turn
            if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
                self.forward = !self.forward;
            }

            // W / Up = pedal forward
            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                if self.forward {
                    self.progress += move_frac;
                } else {
                    self.progress -= move_frac;
                }
            }

            let turn_pref =
                if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                    TurnPreference::Left
                } else if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                    TurnPreference::Right
                } else {
                    TurnPreference::Straight
                };

            if self.progress >= 1.0 {
                self.progress = 1.0;
                arrive_at_node(self, network, edge.node_b, turn_pref);
            } else if self.progress <= 0.0 {
                self.progress = 0.0;
                arrive_at_node(self, network, edge.node_a, turn_pref);
            }
        }

        // Camera: sit on the road
        let edge = &network.edges[self.current_edge as usize];
        let pos = edge.sample_position(self.progress);

        let ahead_frac = (3.0 / edge.length_m).clamp(0.005, 0.15);
        let ahead_progress = if self.forward {
            (self.progress + ahead_frac).min(1.0)
        } else {
            (self.progress - ahead_frac).max(0.0)
        };
        let ahead = edge.sample_position(ahead_progress);

        let eye = Vec3::new(pos[0], 3.0, pos[1]);
        let target = Vec3::new(ahead[0], 2.8, ahead[1]);

        self.free_pos = eye;

        (eye, target)
    }

    fn free_roam_update(&mut self, keys: &KeyState, dt: f32) -> (Vec3, Vec3) {
        let sprint = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            3.0
        } else {
            1.0
        };
        let speed = self.speed * sprint;

        let turn_speed = 2.0;
        if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
            self.free_yaw += turn_speed * dt;
        }
        if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
            self.free_yaw -= turn_speed * dt;
        }

        let pitch_speed = 1.5;
        if keys.pressed(KeyCode::KeyT) {
            self.free_pitch = (self.free_pitch + pitch_speed * dt).min(1.2);
        }
        if keys.pressed(KeyCode::KeyG) {
            self.free_pitch = (self.free_pitch - pitch_speed * dt).max(-1.2);
        }

        let forward = Vec3::new(-self.free_yaw.sin(), 0.0, self.free_yaw.cos());
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

        self.free_pos += velocity;

        // Look direction from yaw + pitch
        let look_dir = Vec3::new(
            -self.free_yaw.sin() * self.free_pitch.cos(),
            self.free_pitch.sin(),
            self.free_yaw.cos() * self.free_pitch.cos(),
        );

        let eye = self.free_pos;
        let target = eye + look_dir * 10.0;

        (eye, target)
    }
}

/// Called when the player reaches a node. Always auto-continues.
fn arrive_at_node(
    state: &mut NavigationState,
    network: &RoadNetwork,
    node_idx: u32,
    turn_pref: TurnPreference,
) {
    let node = &network.nodes[node_idx as usize];
    let current_edge = state.current_edge;

    let other_edges: Vec<u32> = node
        .edge_ids
        .iter()
        .copied()
        .filter(|&e| e != current_edge)
        .collect();

    if other_edges.is_empty() {
        state.forward = !state.forward;
        return;
    }

    if other_edges.len() == 1 {
        transition_to_edge(state, network, other_edges[0], node_idx);
        return;
    }

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
        match turn_pref {
            TurnPreference::Straight => {
                if !roundabout_edges.is_empty() {
                    let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
                    let best = find_straightest_turn(state, network, &sorted, node_idx);
                    sorted[best]
                } else {
                    pick_straightest(state, network, &other_edges, node_idx)
                }
            }
            TurnPreference::Left | TurnPreference::Right => {
                if !normal_edges.is_empty() {
                    let sorted = sort_turns_by_angle(state, network, &normal_edges, node_idx);
                    match turn_pref {
                        TurnPreference::Left => sorted[0],
                        _ => *sorted.last().unwrap(),
                    }
                } else if !roundabout_edges.is_empty() {
                    let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
                    let best = find_straightest_turn(state, network, &sorted, node_idx);
                    sorted[best]
                } else {
                    pick_straightest(state, network, &other_edges, node_idx)
                }
            }
        }
    } else if !roundabout_edges.is_empty() {
        let sorted = sort_turns_by_angle(state, network, &roundabout_edges, node_idx);
        let best = find_straightest_turn(state, network, &sorted, node_idx);
        sorted[best]
    } else {
        match turn_pref {
            TurnPreference::Left => {
                let sorted = sort_turns_by_angle(state, network, &other_edges, node_idx);
                sorted[0]
            }
            TurnPreference::Right => {
                let sorted = sort_turns_by_angle(state, network, &other_edges, node_idx);
                *sorted.last().unwrap()
            }
            TurnPreference::Straight => {
                pick_straightest(state, network, &other_edges, node_idx)
            }
        }
    };

    transition_to_edge(state, network, choice, node_idx);
}

fn pick_straightest(
    state: &NavigationState,
    network: &RoadNetwork,
    edges: &[u32],
    node_idx: u32,
) -> u32 {
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
    let sorted = sort_turns_by_angle(state, network, edges, node_idx);
    let best = find_straightest_turn(state, network, &sorted, node_idx);
    sorted[best]
}

fn transition_to_edge(
    state: &mut NavigationState,
    network: &RoadNetwork,
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

fn sort_turns_by_angle(
    state: &NavigationState,
    network: &RoadNetwork,
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

    edge_angles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    edge_angles.into_iter().map(|(idx, _)| idx).collect()
}

fn find_straightest_turn(
    state: &NavigationState,
    network: &RoadNetwork,
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

fn incoming_direction(
    edge: &sykla_world::road_network::RoadEdge,
    forward: bool,
) -> [f32; 2] {
    let frac = (5.0 / edge.length_m).clamp(0.01, 0.3);
    if forward {
        let d = edge.sample_direction((1.0 - frac).max(0.0));
        [d[0], d[1]]
    } else {
        let d = edge.sample_direction(frac.min(1.0));
        [-d[0], -d[1]]
    }
}

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
