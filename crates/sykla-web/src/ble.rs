use serde::Deserialize;

/// Unified BLE data from all connected cycling peripherals.
/// Polled each frame by the game loop.
#[derive(Debug, Default)]
pub struct BleState {
    // Resolved sensor data (priority-resolved on JS side)
    pub speed_kmh: f64,
    pub power_watts: f64,
    pub cadence_rpm: f64,
    pub heart_rate_bpm: Option<u16>,

    // Per-device connection status
    pub trainer_connected: bool,
    pub hrm_connected: bool,
    pub power_connected: bool,
    pub csc_connected: bool,

    // Grade smoothing for trainer commands
    smoothed_grade: f64,
    last_trainer_send_ms: f64,
}

/// JSON shape returned by syklaBle.getLatestData()
#[derive(Deserialize)]
struct BleDataJson {
    speed_kmh: f64,
    power_watts: f64,
    cadence_rpm: f64,
    heart_rate_bpm: Option<u16>,
    trainer_connected: bool,
    hrm_connected: bool,
    power_connected: bool,
    csc_connected: bool,
}

// Web Bluetooth interop via wasm-bindgen
mod web_ble {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        // Data polling
        #[wasm_bindgen(js_namespace = syklaBle, js_name = getLatestData)]
        pub fn get_latest_data() -> Option<String>;

        #[wasm_bindgen(js_namespace = syklaBle, js_name = isConnected)]
        pub fn is_connected() -> bool;

        // Connection status per device type
        #[wasm_bindgen(js_namespace = syklaBle, js_name = isTrainerConnected)]
        pub fn is_trainer_connected() -> bool;

        #[wasm_bindgen(js_namespace = syklaBle, js_name = isHRMConnected)]
        pub fn is_hrm_connected() -> bool;

        // Trainer commands (FTMS Control Point)
        #[wasm_bindgen(js_namespace = syklaBle, js_name = setSimulationParameters)]
        pub fn set_simulation_parameters(wind_speed: f64, grade: f64, crr: f64, cda: f64);

        #[wasm_bindgen(js_namespace = syklaBle, js_name = setTargetPower)]
        pub fn set_target_power(watts: f64);

        #[wasm_bindgen(js_namespace = syklaBle, js_name = setResistanceLevel)]
        pub fn set_resistance_level(level: f64);

        // Legacy: raw byte write to control point
        #[wasm_bindgen(js_namespace = syklaBle, js_name = writeSimParams)]
        pub fn write_sim_params(data: &[u8]);
    }
}

const TRAINER_SEND_INTERVAL_MS: f64 = 1000.0;

impl BleState {
    /// Poll all BLE device data. Call once per frame.
    pub fn poll(&mut self) {
        if let Some(json) = web_ble::get_latest_data() {
            if let Ok(data) = serde_json::from_str::<BleDataJson>(&json) {
                self.speed_kmh = data.speed_kmh;
                self.power_watts = data.power_watts;
                self.cadence_rpm = data.cadence_rpm;
                self.heart_rate_bpm = data.heart_rate_bpm;
                self.trainer_connected = data.trainer_connected;
                self.hrm_connected = data.hrm_connected;
                self.power_connected = data.power_connected;
                self.csc_connected = data.csc_connected;
            }
        }
    }

    /// Returns true if any device is connected.
    pub fn any_connected(&self) -> bool {
        self.trainer_connected || self.hrm_connected || self.power_connected || self.csc_connected
    }

    /// Send simulation parameters to the trainer at ~1Hz with grade smoothing.
    /// `grade_percent` is the raw route gradient. `now_ms` is the current time.
    /// `crr` and `cda` come from the physics params.
    pub fn send_sim_params(
        &mut self,
        grade_percent: f64,
        now_ms: f64,
        crr: f64,
        cda: f64,
    ) {
        if !self.trainer_connected {
            return;
        }

        // Throttle to ~1Hz
        if now_ms - self.last_trainer_send_ms < TRAINER_SEND_INTERVAL_MS {
            return;
        }
        self.last_trainer_send_ms = now_ms;

        // EMA smooth the grade to avoid jarring resistance shifts
        let alpha = 0.4; // ~0.5s time constant at 1Hz
        self.smoothed_grade += (grade_percent - self.smoothed_grade) * alpha;

        web_ble::set_simulation_parameters(0.0, self.smoothed_grade, crr, cda);
    }

    /// Set ERG mode target power (for structured workouts).
    pub fn set_target_power(&self, watts: f64) {
        if self.trainer_connected {
            web_ble::set_target_power(watts);
        }
    }

    /// Set direct resistance level (0.0 to 1.0).
    pub fn set_resistance(&self, level: f64) {
        if self.trainer_connected {
            web_ble::set_resistance_level(level);
        }
    }
}
