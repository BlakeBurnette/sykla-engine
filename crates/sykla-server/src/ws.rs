use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppState;

// ---------------------------------------------------------------------------
// Global sync types (cross-world visibility)
// ---------------------------------------------------------------------------

/// A geographic position update from an outdoor rider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GlobalClientMessage {
    #[serde(rename = "geo_position")]
    GeoPosition {
        lat: f64,
        lng: f64,
        speed_kmh: f64,
        heading: f64,
        display_name: Option<String>,
        is_indoor: bool,
    },
}

/// Server broadcast of nearby riders in the global channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GlobalServerMessage {
    #[serde(rename = "nearby_riders")]
    NearbyRiders { riders: Vec<GlobalRiderPosition> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRiderPosition {
    pub user_id: String,
    pub display_name: String,
    pub lat: f64,
    pub lng: f64,
    pub speed_kmh: f64,
    pub heading: f64,
    pub is_indoor: bool,
}

/// Global room for cross-world visibility.
pub struct GlobalRoom {
    pub tx: broadcast::Sender<String>,
    pub riders: DashMap<Uuid, GlobalRiderPosition>,
}

impl GlobalRoom {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            riders: DashMap::new(),
        }
    }
}

/// Shared global room instance.
pub type GlobalRoomHandle = Arc<GlobalRoom>;

/// A position update from a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "position")]
    Position {
        lat: Option<f64>,
        lng: Option<f64>,
        distance_m: f64,
        speed_kmh: f64,
        is_indoor: bool,
    },
}

/// A message sent from server to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "riders")]
    Riders { positions: Vec<RiderPosition> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiderPosition {
    pub user_id: String,
    pub display_name: String,
    pub distance_m: f64,
    pub speed_kmh: f64,
    pub is_indoor: bool,
}

/// Per-route room that holds connected riders' state and a broadcast channel.
pub struct RouteRoom {
    pub tx: broadcast::Sender<String>,
    pub riders: DashMap<Uuid, RiderPosition>,
}

impl RouteRoom {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            riders: DashMap::new(),
        }
    }
}

/// Global state for all route rooms.
pub type RoomManager = Arc<DashMap<Uuid, Arc<RouteRoom>>>;

pub fn new_room_manager() -> RoomManager {
    Arc::new(DashMap::new())
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(route_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, route_id, state))
}

async fn handle_socket(socket: WebSocket, route_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Get or create the room for this route
    let room = state
        .rooms
        .entry(route_id)
        .or_insert_with(|| Arc::new(RouteRoom::new()))
        .clone();

    // Generate a temporary user ID for this connection
    // In production, this would come from JWT auth on the WS handshake
    let user_id = Uuid::new_v4();
    let display_name = format!("Rider-{}", &user_id.to_string()[..4]);

    // Subscribe to room broadcasts
    let mut rx = room.tx.subscribe();

    // Spawn task to forward broadcast messages to this client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Position {
                            distance_m,
                            speed_kmh,
                            is_indoor,
                            ..
                        } => {
                            // Update this rider's position
                            room.riders.insert(
                                user_id,
                                RiderPosition {
                                    user_id: user_id.to_string(),
                                    display_name: display_name.clone(),
                                    distance_m,
                                    speed_kmh,
                                    is_indoor,
                                },
                            );

                            // Broadcast all positions to the room
                            let positions: Vec<RiderPosition> =
                                room.riders.iter().map(|r| r.value().clone()).collect();

                            let server_msg = ServerMessage::Riders { positions };
                            if let Ok(json) = serde_json::to_string(&server_msg) {
                                let _ = room.tx.send(json);
                            }
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Clean up on disconnect
    room.riders.remove(&user_id);
    send_task.abort();

    // If room is empty, remove it
    if room.riders.is_empty() {
        state.rooms.remove(&route_id);
    }
}

// ---------------------------------------------------------------------------
// Global sync WebSocket handler (cross-world visibility)
// ---------------------------------------------------------------------------

pub fn new_global_room() -> GlobalRoomHandle {
    Arc::new(GlobalRoom::new())
}

/// WebSocket upgrade handler for global sync.
pub async fn global_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_global_socket(socket, state))
}

/// Default proximity radius for nearby rider broadcasting (meters).
const PROXIMITY_RADIUS_M: f64 = 5000.0;

async fn handle_global_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    let room = &state.global_room;
    let user_id = Uuid::new_v4();
    let display_name = format!("Rider-{}", &user_id.to_string()[..4]);

    let mut rx = room.tx.subscribe();

    // Forward broadcasts to this client
    let user_id_clone = user_id;
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(client_msg) = serde_json::from_str::<GlobalClientMessage>(&text) {
                    match client_msg {
                        GlobalClientMessage::GeoPosition {
                            lat,
                            lng,
                            speed_kmh,
                            heading,
                            display_name: name_override,
                            is_indoor,
                        } => {
                            let rider_name =
                                name_override.unwrap_or_else(|| display_name.clone());

                            room.riders.insert(
                                user_id,
                                GlobalRiderPosition {
                                    user_id: user_id.to_string(),
                                    display_name: rider_name,
                                    lat,
                                    lng,
                                    speed_kmh,
                                    heading,
                                    is_indoor,
                                },
                            );

                            // Broadcast nearby riders to all (filtered by proximity)
                            let my_pos = (lat, lng);
                            let nearby: Vec<GlobalRiderPosition> = room
                                .riders
                                .iter()
                                .filter(|r| {
                                    let d = sykla_core::geo_math::haversine_distance(
                                        my_pos.0, my_pos.1, r.lat, r.lng,
                                    );
                                    d <= PROXIMITY_RADIUS_M
                                })
                                .map(|r| r.value().clone())
                                .collect();

                            let server_msg = GlobalServerMessage::NearbyRiders { riders: nearby };
                            if let Ok(json) = serde_json::to_string(&server_msg) {
                                let _ = room.tx.send(json);
                            }
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Clean up
    room.riders.remove(&user_id_clone);
    send_task.abort();
}
