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
