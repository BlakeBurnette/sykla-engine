use bevy::prelude::*;
use sykla_core::ftms::{IndoorBikeData, SimulationParams};

/// Resource holding the current trainer connection state and data.
#[derive(Resource, Debug, Default)]
pub struct TrainerState {
    pub connected: bool,
    pub speed_kmh: f64,
    pub power_watts: f64,
    pub cadence_rpm: f64,
    pub heart_rate_bpm: Option<u8>,
}

/// Resource holding the simulation parameters to send to the trainer.
#[derive(Resource, Debug)]
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

// Web Bluetooth interop:
// In WASM builds, these functions call JavaScript via wasm-bindgen.
// In native builds, they are stubs.

#[cfg(target_arch = "wasm32")]
mod web_ble {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        /// Request a BLE device with FTMS service.
        /// Returns a promise that resolves when connected.
        #[wasm_bindgen(js_namespace = syklaBle, js_name = connectTrainer)]
        pub fn connect_trainer() -> js_sys::Promise;

        /// Disconnect from the trainer.
        #[wasm_bindgen(js_namespace = syklaBle, js_name = disconnectTrainer)]
        pub fn disconnect_trainer();

        /// Check if trainer is connected.
        #[wasm_bindgen(js_namespace = syklaBle, js_name = isConnected)]
        pub fn is_connected() -> bool;

        /// Get the latest indoor bike data as JSON string.
        #[wasm_bindgen(js_namespace = syklaBle, js_name = getLatestData)]
        pub fn get_latest_data() -> Option<String>;

        /// Write simulation parameters to the trainer.
        #[wasm_bindgen(js_namespace = syklaBle, js_name = writeSimParams)]
        pub fn write_sim_params(data: &[u8]);
    }
}

/// Bevy plugin for trainer BLE integration.
pub struct TrainerPlugin;

impl Plugin for TrainerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainerState>()
            .init_resource::<TrainerCommand>()
            .add_systems(Update, (poll_trainer_data, send_trainer_commands));
    }
}

/// Poll the trainer for new data each frame.
fn poll_trainer_data(mut trainer: ResMut<TrainerState>) {
    #[cfg(target_arch = "wasm32")]
    {
        trainer.connected = web_ble::is_connected();

        if let Some(json) = web_ble::get_latest_data() {
            if let Ok(data) = serde_json::from_str::<IndoorBikeData>(&json) {
                trainer.speed_kmh = data.speed_kmh;
                trainer.power_watts = data.power_watts;
                trainer.cadence_rpm = data.cadence_rpm;
                trainer.heart_rate_bpm = data.heart_rate_bpm;
            }
        }
    }

    // For non-WASM (testing), simulate some data
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = &mut *trainer; // suppress unused warning
    }
}

/// Send updated simulation parameters to the trainer when dirty.
fn send_trainer_commands(mut cmd: ResMut<TrainerCommand>) {
    if !cmd.dirty {
        return;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let encoded = cmd.params.encode();
        web_ble::write_sim_params(&encoded);
    }

    cmd.dirty = false;
}
