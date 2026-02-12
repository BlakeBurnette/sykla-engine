use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub display_name: String,
    pub weight_kg: Option<f32>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RouteRecord {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub gpx_data: String,
    pub distance_m: f32,
    pub elevation_gain_m: f32,
    pub min_elevation_m: Option<f32>,
    pub max_elevation_m: Option<f32>,
    pub start_lat: Option<f64>,
    pub start_lng: Option<f64>,
    pub city: Option<String>,
    pub is_public: Option<bool>,
    pub source: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RoutePointRecord {
    pub id: i64,
    pub route_id: Uuid,
    pub seq: i32,
    pub lat: f64,
    pub lng: f64,
    pub elevation_m: f32,
    pub distance_from_start_m: f32,
    pub grade_percent: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ride {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub route_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i32>,
    pub avg_power_watts: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub avg_cadence_rpm: Option<f32>,
    pub calories: Option<i32>,
    pub is_indoor: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Summary of a route for list views (no GPX data).
#[derive(Debug, Serialize, Deserialize)]
pub struct RouteSummary {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub distance_m: f32,
    pub elevation_gain_m: f32,
    pub city: Option<String>,
    pub is_public: Option<bool>,
    pub source: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
