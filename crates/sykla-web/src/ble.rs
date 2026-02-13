use sykla_core::ftms::{IndoorBikeData, SimulationParams};

/// Trainer connection state and data — plain struct, no Bevy.
#[derive(Debug, Default)]
pub struct TrainerState {
    pub connected: bool,
    pub speed_kmh: f64,
    pub power_watts: f64,
    pub cadence_rpm: f64,
    pub heart_rate_bpm: Option<u8>,
}

/// Trainer commands — plain struct, no Bevy.
#[derive(Debug)]
pub struct TrainerCommand {
    pub params: SimulationParams,
    pub dirty: bool,
}

impl Default for TrainerCommand {
    fn default() -> Self {
        Self {
            params: SimulationParams::default(),
            dirty: false,
        }
    }
}

// Web Bluetooth interop via wasm-bindgen
mod web_ble {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = syklaBle, js_name = connectTrainer)]
        pub fn connect_trainer() -> js_sys::Promise;

        #[wasm_bindgen(js_namespace = syklaBle, js_name = disconnectTrainer)]
        pub fn disconnect_trainer();

        #[wasm_bindgen(js_namespace = syklaBle, js_name = isConnected)]
        pub fn is_connected() -> bool;

        #[wasm_bindgen(js_namespace = syklaBle, js_name = getLatestData)]
        pub fn get_latest_data() -> Option<String>;

        #[wasm_bindgen(js_namespace = syklaBle, js_name = writeSimParams)]
        pub fn write_sim_params(data: &[u8]);
    }
}

impl TrainerState {
    /// Poll the trainer for new data. Call each frame.
    pub fn poll(&mut self) {
        self.connected = web_ble::is_connected();

        if let Some(json) = web_ble::get_latest_data() {
            if let Ok(data) = serde_json::from_str::<IndoorBikeData>(&json) {
                self.speed_kmh = data.speed_kmh;
                self.power_watts = data.power_watts;
                self.cadence_rpm = data.cadence_rpm;
                self.heart_rate_bpm = data.heart_rate_bpm;
            }
        }
    }
}

impl TrainerCommand {
    /// Send updated simulation parameters to the trainer when dirty.
    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        let encoded = self.params.encode();
        web_ble::write_sim_params(&encoded);

        self.dirty = false;
    }
}
