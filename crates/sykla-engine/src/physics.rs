use crate::terrain::{RoutePoint, SurfaceType, surface_at};

pub struct PhysicsParams {
    pub mass_kg: f32,
    pub crr: f32,
    pub cda: f32,
    pub rho: f32,
    pub max_speed: f32,
    pub min_speed: f32,
}

impl Default for PhysicsParams {
    fn default() -> Self {
        Self {
            mass_kg: 80.0,
            crr: 0.005,
            cda: 0.32,
            rho: 1.225,
            max_speed: 19.4,
            min_speed: 1.0,
        }
    }
}

pub struct PhysicsState {
    pub speed: f32,
    pub distance: f32,
    pub total_distance: f32,
    pub gradient: f32,
    pub power_watts: f32,
    pub elapsed_secs: f32,
    pub heart_rate: u32,
    pub cadence: u32,
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self {
            speed: 8.0,
            distance: 0.0,
            total_distance: 0.0,
            gradient: 0.0,
            power_watts: 200.0,
            elapsed_secs: 0.0,
            heart_rate: 145,
            cadence: 85,
        }
    }
}

impl PhysicsState {
    pub fn speed_kmh(&self) -> f32 {
        self.speed * 3.6
    }

    pub fn gradient_percent(&self) -> f32 {
        self.gradient * 100.0
    }

    pub fn distance_km(&self) -> f32 {
        self.distance / 1000.0
    }

    pub fn elapsed_display(&self) -> (u32, u32) {
        let total_secs = self.elapsed_secs as u32;
        (total_secs / 60, total_secs % 60)
    }
}

fn elevation_at_distance(points: &[RoutePoint], d: f64) -> f64 {
    if points.is_empty() {
        return 100.0;
    }
    if d <= points[0].distance_m {
        return points[0].elevation_m;
    }
    for i in 1..points.len() {
        if points[i].distance_m >= d {
            let prev = &points[i - 1];
            let curr = &points[i];
            let t = (d - prev.distance_m) / (curr.distance_m - prev.distance_m);
            return prev.elevation_m + t * (curr.elevation_m - prev.elevation_m);
        }
    }
    points.last().unwrap().elevation_m
}

pub fn gradient_at(points: &[RoutePoint], distance: f32) -> f32 {
    let d = distance as f64;
    let half = 10.0;
    let elev_before = elevation_at_distance(points, d - half);
    let elev_after = elevation_at_distance(points, d + half);
    ((elev_after - elev_before) / (2.0 * half)) as f32
}

const G: f32 = 9.81;

pub fn update_physics(
    state: &mut PhysicsState,
    params: &PhysicsParams,
    points: &[RoutePoint],
    route_length: f32,
    dt: f32,
) {
    state.elapsed_secs += dt;
    state.gradient = gradient_at(points, state.distance);

    let v = state.speed.max(params.min_speed);

    // Gravel increases rolling resistance (~5% more power at typical speeds)
    let crr = match surface_at(points, state.distance as f64) {
        SurfaceType::Gravel => params.crr * 1.15,
        SurfaceType::Paved => params.crr,
    };

    let f_gravity = params.mass_kg * G * state.gradient;
    let f_rolling = crr * params.mass_kg * G;
    let f_aero = 0.5 * params.cda * params.rho * v * v;
    let f_drive = state.power_watts / v;

    let f_net = f_drive - f_gravity - f_rolling - f_aero;
    let accel = f_net / params.mass_kg;
    state.speed = (state.speed + accel * dt).clamp(params.min_speed, params.max_speed);
    let step = state.speed * dt;
    state.distance += step;
    state.total_distance += step;

    if state.distance > route_length {
        state.distance -= route_length;
    }
}
