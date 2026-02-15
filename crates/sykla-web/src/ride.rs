use sykla_core::ride_engine::RideEngine;
use sykla_core::types::Route;

use crate::ble::BleState;

/// Active ride state — plain struct wrapping RideEngine.
pub struct ActiveRide {
    pub engine: RideEngine,
}

impl ActiveRide {
    pub fn new(route: Route) -> Self {
        Self {
            engine: RideEngine::new(route),
        }
    }

    /// Update ride simulation and send grade to trainer. Returns current grade.
    pub fn update(&mut self, dt: f64, ble: &mut BleState, t: f32, crr: f64, cda: f64) -> f64 {
        let grade = self.engine.update(
            dt,
            ble.speed_kmh,
            ble.power_watts,
            ble.cadence_rpm,
        );

        // Send grade to trainer at ~1Hz
        let now_ms = (t * 1000.0) as f64;
        ble.send_sim_params(grade, now_ms, crr, cda);

        grade
    }
}
