use crate::gradient::{elevation_at_distance, grade_at_distance};
use crate::types::{RideState, Route};

/// The ride engine: given a route and speed updates, tracks virtual position
/// and provides the current grade for trainer resistance control.
pub struct RideEngine {
    route: Route,
    state: RideState,
}

impl RideEngine {
    pub fn new(route: Route) -> Self {
        let initial_elevation = route
            .points
            .first()
            .map(|p| p.elevation_m)
            .unwrap_or(0.0);

        Self {
            route,
            state: RideState {
                current_elevation_m: initial_elevation,
                ..RideState::new()
            },
        }
    }

    /// Update the ride state with a time delta and current trainer data.
    /// `dt_secs` — time since last update in seconds.
    /// `speed_kmh` — current speed from trainer.
    /// `power_watts` — current power from trainer.
    /// `cadence_rpm` — current cadence from trainer.
    ///
    /// Returns the grade the trainer should be set to.
    pub fn update(
        &mut self,
        dt_secs: f64,
        speed_kmh: f64,
        power_watts: f64,
        cadence_rpm: f64,
    ) -> f64 {
        if self.state.is_complete {
            return 0.0;
        }

        // Convert speed to m/s and advance distance
        let speed_ms = speed_kmh / 3.6;
        let distance_delta = speed_ms * dt_secs;
        self.state.distance_m += distance_delta;
        self.state.elapsed_secs += dt_secs;
        self.state.current_speed_kmh = speed_kmh;
        self.state.current_power_watts = power_watts;
        self.state.current_cadence_rpm = cadence_rpm;

        // Check if ride is complete
        if self.state.distance_m >= self.route.total_distance_m {
            self.state.distance_m = self.route.total_distance_m;
            self.state.is_complete = true;
        }

        // Look up current grade and elevation
        let grade = grade_at_distance(&self.route.points, self.state.distance_m);
        let elevation = elevation_at_distance(&self.route.points, self.state.distance_m);
        self.state.current_grade_percent = grade;
        self.state.current_elevation_m = elevation;

        grade
    }

    /// Get current ride state (immutable reference).
    pub fn state(&self) -> &RideState {
        &self.state
    }

    /// Get the route being ridden.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Get current position as (lat, lng) along the route.
    pub fn current_position(&self) -> Option<(f64, f64)> {
        self.route.position_at_distance(self.state.distance_m)
    }

    /// Get progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.route.total_distance_m <= 0.0 {
            return 0.0;
        }
        (self.state.distance_m / self.route.total_distance_m).min(1.0)
    }

    /// Get distance remaining in meters.
    pub fn distance_remaining_m(&self) -> f64 {
        (self.route.total_distance_m - self.state.distance_m).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpx_parser::parse_gpx;

    fn sample_gpx() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test"
  xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <name>Test Ride Route</name>
    <trkseg>
      <trkpt lat="35.7796" lon="-78.6382"><ele>100.0</ele></trkpt>
      <trkpt lat="35.7800" lon="-78.6380"><ele>105.0</ele></trkpt>
      <trkpt lat="35.7805" lon="-78.6375"><ele>115.0</ele></trkpt>
      <trkpt lat="35.7810" lon="-78.6370"><ele>110.0</ele></trkpt>
      <trkpt lat="35.7815" lon="-78.6365"><ele>108.0</ele></trkpt>
      <trkpt lat="35.7820" lon="-78.6360"><ele>120.0</ele></trkpt>
    </trkseg>
  </trk>
</gpx>"#
    }

    #[test]
    fn test_ride_engine_initial_state() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let engine = RideEngine::new(route);
        assert_eq!(engine.state().distance_m, 0.0);
        assert_eq!(engine.state().elapsed_secs, 0.0);
        assert!(!engine.state().is_complete);
        assert!((engine.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ride_engine_advance() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let total = route.total_distance_m;
        let mut engine = RideEngine::new(route);

        // Ride at 30 km/h for 1 second = ~8.33m
        let grade = engine.update(1.0, 30.0, 200.0, 80.0);
        assert!(grade.is_finite());
        assert!(engine.state().distance_m > 0.0);
        assert_eq!(engine.state().elapsed_secs, 1.0);
        assert!(engine.progress() > 0.0);
        assert!(engine.distance_remaining_m() < total);
    }

    #[test]
    fn test_ride_engine_completion() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let total = route.total_distance_m;
        let mut engine = RideEngine::new(route);

        // Ride far enough to complete
        let big_speed = 360.0; // 100 m/s
        let time_needed = total / 100.0 + 1.0;
        engine.update(time_needed, big_speed, 200.0, 80.0);

        assert!(engine.state().is_complete);
        assert!((engine.progress() - 1.0).abs() < 0.001);
        assert!((engine.distance_remaining_m()).abs() < 0.1);
    }

    #[test]
    fn test_ride_engine_position_tracking() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let mut engine = RideEngine::new(route);

        let pos_start = engine.current_position().unwrap();
        engine.update(1.0, 30.0, 200.0, 80.0);
        let pos_after = engine.current_position().unwrap();

        // Position should have changed
        assert!(
            (pos_start.0 - pos_after.0).abs() > 0.00001
                || (pos_start.1 - pos_after.1).abs() > 0.00001
        );
    }

    #[test]
    fn test_ride_engine_returns_grade() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let mut engine = RideEngine::new(route);

        // First segment goes from 100m to 105m — uphill
        let grade = engine.update(0.1, 30.0, 200.0, 80.0);
        assert!(grade.is_finite());
    }
}
