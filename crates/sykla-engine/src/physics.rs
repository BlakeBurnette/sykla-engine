use crate::terrain::{RoutePoint, SurfaceType, surface_at};

/// Riding position affects aerodynamic drag (CdA).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RidingPosition {
    Tops,     // sitting up, hands on bar top
    Hoods,    // hands on brake hoods, moderate tuck
    Drops,    // hands in drops, elbows bent, tucked
    AeroBars, // time trial position, forearms on aero bars
}

impl RidingPosition {
    /// CdA (m²) for a ~75kg, 180cm rider on a road bike.
    pub fn cda(self) -> f32 {
        match self {
            RidingPosition::Tops => 0.400,
            RidingPosition::Hoods => 0.324,
            RidingPosition::Drops => 0.295,
            RidingPosition::AeroBars => 0.220,
        }
    }
}

pub struct PhysicsParams {
    pub rider_mass_kg: f32,
    pub bike_mass_kg: f32,
    pub crr: f32,
    pub position: RidingPosition,
    pub drivetrain_efficiency: f32,
    pub max_speed: f32,
    pub min_speed: f32,
}

impl PhysicsParams {
    pub fn total_mass(&self) -> f32 {
        self.rider_mass_kg + self.bike_mass_kg
    }
}

impl Default for PhysicsParams {
    fn default() -> Self {
        Self {
            rider_mass_kg: 72.0,
            bike_mass_kg: 8.0,
            crr: 0.004,
            position: RidingPosition::Drops,
            drivetrain_efficiency: 0.97,
            max_speed: 27.8, // 100 km/h — pro descents
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
    pub elevation: f32,
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
            elevation: 0.0,
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

/// Air density at altitude and temperature (ISA barometric formula).
/// Standard sea level: 1.225 kg/m³ at 15°C.
fn air_density(elevation_m: f32, temp_celsius: f32) -> f32 {
    let rho_0 = 1.225_f32;
    let rho_alt = rho_0 * (1.0 - 0.0000226 * elevation_m).powf(5.256);
    // Temperature correction (ISA standard = 15°C = 288.15 K)
    rho_alt * (288.15 / (273.15 + temp_celsius))
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
    state.elevation = elevation_at_distance(points, state.distance as f64) as f32;

    let v = state.speed.max(params.min_speed);
    let mass = params.total_mass();
    let cda = params.position.cda();

    // Surface-dependent rolling resistance
    let crr = match surface_at(points, state.distance as f64) {
        SurfaceType::Gravel => params.crr * 1.6, // packed gravel
        SurfaceType::Paved => params.crr,
    };

    // Altitude-adjusted air density (assume 15°C)
    let rho = air_density(state.elevation, 15.0);

    let f_gravity = mass * G * state.gradient;
    let f_rolling = crr * mass * G;
    let f_aero = 0.5 * cda * rho * v * v;
    let f_drive = (state.power_watts * params.drivetrain_efficiency) / v;

    let f_net = f_drive - f_gravity - f_rolling - f_aero;
    let accel = f_net / mass;
    state.speed = (state.speed + accel * dt).clamp(params.min_speed, params.max_speed);
    let step = state.speed * dt;
    state.distance += step;
    state.total_distance += step;

    if state.distance > route_length {
        state.distance -= route_length;
    }
}
