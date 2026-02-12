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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CityRecord {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub center_lat: f64,
    pub center_lng: f64,
    pub bbox_south: Option<f64>,
    pub bbox_west: Option<f64>,
    pub bbox_north: Option<f64>,
    pub bbox_east: Option<f64>,
    pub file_size_bytes: i64,
    pub gcs_bucket: String,
    pub gcs_object: String,
    pub download_url: String,
    pub version: i32,
    pub created_at: Option<DateTime<Utc>>,
}

/// Summary of a city for API responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct CitySummary {
    pub slug: String,
    pub name: String,
    pub center_lat: f64,
    pub center_lng: f64,
    pub file_size_bytes: i64,
    pub download_url: String,
    pub version: i32,
}

// ---------------------------------------------------------------------------
// Activity models
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActivityRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub source: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<f32>,
    pub distance_m: Option<f32>,
    pub elevation_gain_m: Option<f32>,
    pub avg_power_watts: Option<f32>,
    pub max_power_watts: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub max_speed_kmh: Option<f32>,
    pub avg_heart_rate_bpm: Option<i16>,
    pub max_heart_rate_bpm: Option<i16>,
    pub avg_cadence_rpm: Option<f32>,
    pub max_cadence_rpm: Option<f32>,
    pub calories: Option<i32>,
    pub bbox_south: Option<f64>,
    pub bbox_west: Option<f64>,
    pub bbox_north: Option<f64>,
    pub bbox_east: Option<f64>,
    pub generated_route_id: Option<Uuid>,
    pub gpx_data: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActivityPointRecord {
    pub id: i64,
    pub activity_id: Uuid,
    pub seq: i32,
    pub timestamp_ms: i64,
    pub lat: f64,
    pub lng: f64,
    pub elevation_m: Option<f32>,
    pub speed_kmh: Option<f32>,
    pub power_watts: Option<f32>,
    pub heart_rate_bpm: Option<i16>,
    pub cadence_rpm: Option<f32>,
    pub distance_from_start_m: f32,
    pub grade_percent: Option<f32>,
}

/// Summary of an activity for list views (no points).
#[derive(Debug, Serialize, Deserialize)]
pub struct ActivitySummary {
    pub id: Uuid,
    pub name: String,
    pub source: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<f32>,
    pub distance_m: Option<f32>,
    pub elevation_gain_m: Option<f32>,
    pub avg_power_watts: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub avg_heart_rate_bpm: Option<i16>,
    pub avg_cadence_rpm: Option<f32>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct HeatmapCellRecord {
    pub zoom: i32,
    pub grid_x: i32,
    pub grid_y: i32,
    pub ride_count: i32,
    pub unique_riders: i32,
    pub total_points: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DetectedCorridorRecord {
    pub id: Uuid,
    pub popularity_score: f32,
    pub is_loop: bool,
    pub distance_m: Option<f32>,
    pub polyline_json: String,
    pub promoted_route_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
}
