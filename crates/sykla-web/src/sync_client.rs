use bevy::prelude::*;
use sykla_core::types::UserPosition;

/// Resource for WebSocket sync state.
#[derive(Resource, Default)]
pub struct SyncState {
    pub connected: bool,
    pub route_id: Option<String>,
    pub other_riders: Vec<UserPosition>,
}

/// Plugin for real-time position sync over WebSocket.
pub struct SyncPlugin;

impl Plugin for SyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SyncState>();
        // WebSocket connection and position broadcasting will use
        // wasm-bindgen + JS WebSocket API in the WASM build.
        // For MVP, this is a placeholder — sync messages are handled
        // via JS interop similar to the BLE pattern.
    }
}
