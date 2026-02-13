use serde::{Deserialize, Serialize};

/// Road surface type — affects rolling resistance in the ride engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfaceType {
    #[default]
    Asphalt,
    Concrete,
    Gravel,
    Cobblestone,
    Dirt,
    Sand,
    Grass,
    Wet,
}

impl SurfaceType {
    /// Stable integer representation for FFI (C ABI).
    pub fn as_i32(&self) -> i32 {
        match self {
            SurfaceType::Asphalt => 0,
            SurfaceType::Concrete => 1,
            SurfaceType::Gravel => 2,
            SurfaceType::Cobblestone => 3,
            SurfaceType::Dirt => 4,
            SurfaceType::Sand => 5,
            SurfaceType::Grass => 6,
            SurfaceType::Wet => 7,
        }
    }

    /// Rolling resistance coefficient relative to asphalt (1.0).
    pub fn rolling_resistance(&self) -> f64 {
        match self {
            SurfaceType::Asphalt => 1.0,
            SurfaceType::Concrete => 1.05,
            SurfaceType::Gravel => 1.6,
            SurfaceType::Cobblestone => 1.4,
            SurfaceType::Dirt => 1.5,
            SurfaceType::Sand => 2.5,
            SurfaceType::Grass => 1.8,
            SurfaceType::Wet => 1.15,
        }
    }
}

/// A single point along a route with geographic and distance data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePoint {
    pub lat: f64,
    pub lng: f64,
    pub elevation_m: f64,
    pub distance_from_start_m: f64,
    pub grade_percent: Option<f64>,
    #[serde(default)]
    pub surface: SurfaceType,
}

/// A complete route: ordered list of points with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub name: String,
    pub points: Vec<RoutePoint>,
    pub total_distance_m: f64,
    pub elevation_gain_m: f64,
    pub min_elevation_m: f64,
    pub max_elevation_m: f64,
}

/// Current state of a ride in progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideState {
    pub distance_m: f64,
    pub elapsed_secs: f64,
    pub current_grade_percent: f64,
    pub current_elevation_m: f64,
    pub current_speed_kmh: f64,
    pub current_power_watts: f64,
    pub current_cadence_rpm: f64,
    pub is_complete: bool,
}

impl RideState {
    pub fn new() -> Self {
        Self {
            distance_m: 0.0,
            elapsed_secs: 0.0,
            current_grade_percent: 0.0,
            current_elevation_m: 0.0,
            current_speed_kmh: 0.0,
            current_power_watts: 0.0,
            current_cadence_rpm: 0.0,
            is_complete: false,
        }
    }
}

impl Default for RideState {
    fn default() -> Self {
        Self::new()
    }
}

/// Position of a user on a route (for real-time sync).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPosition {
    pub user_id: String,
    pub display_name: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub distance_m: f64,
    pub speed_kmh: f64,
    pub is_indoor: bool,
}
