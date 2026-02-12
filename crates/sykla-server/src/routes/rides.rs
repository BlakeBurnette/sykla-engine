use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{post, put},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::models::Ride;
use crate::AppState;

pub fn rides_router() -> Router<AppState> {
    Router::new()
        .route("/", post(start_ride).get(list_rides))
        .route("/{id}", put(update_ride))
}

#[derive(Deserialize)]
pub struct StartRideRequest {
    pub route_id: Uuid,
    pub is_indoor: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateRideRequest {
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i32>,
    pub avg_power_watts: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub avg_cadence_rpm: Option<f32>,
    pub calories: Option<i32>,
}

#[derive(Serialize)]
pub struct RideResponse {
    pub id: Uuid,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i32>,
    pub avg_power_watts: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub avg_cadence_rpm: Option<f32>,
    pub calories: Option<i32>,
    pub is_indoor: Option<bool>,
}

async fn start_ride(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(req): Json<StartRideRequest>,
) -> Result<(StatusCode, Json<RideResponse>), (StatusCode, String)> {
    let now = Utc::now();
    let ride_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO rides (user_id, route_id, started_at, is_indoor) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.user_id)
    .bind(req.route_id)
    .bind(now)
    .bind(req.is_indoor.unwrap_or(true))
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(RideResponse {
            id: ride_id,
            route_id: req.route_id,
            started_at: now,
            completed_at: None,
            duration_secs: None,
            avg_power_watts: None,
            avg_speed_kmh: None,
            avg_cadence_rpm: None,
            calories: None,
            is_indoor: Some(req.is_indoor.unwrap_or(true)),
        }),
    ))
}

async fn update_ride(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRideRequest>,
) -> Result<Json<RideResponse>, (StatusCode, String)> {
    let ride = sqlx::query_as::<_, Ride>(
        "SELECT * FROM rides WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Ride not found".to_string()))?;

    sqlx::query(
        r#"UPDATE rides SET completed_at = COALESCE($1, completed_at),
           duration_secs = COALESCE($2, duration_secs),
           avg_power_watts = COALESCE($3, avg_power_watts),
           avg_speed_kmh = COALESCE($4, avg_speed_kmh),
           avg_cadence_rpm = COALESCE($5, avg_cadence_rpm),
           calories = COALESCE($6, calories)
           WHERE id = $7"#,
    )
    .bind(req.completed_at)
    .bind(req.duration_secs)
    .bind(req.avg_power_watts)
    .bind(req.avg_speed_kmh)
    .bind(req.avg_cadence_rpm)
    .bind(req.calories)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RideResponse {
        id,
        route_id: ride.route_id.unwrap_or_default(),
        started_at: ride.started_at,
        completed_at: req.completed_at.or(ride.completed_at),
        duration_secs: req.duration_secs.or(ride.duration_secs),
        avg_power_watts: req.avg_power_watts.or(ride.avg_power_watts),
        avg_speed_kmh: req.avg_speed_kmh.or(ride.avg_speed_kmh),
        avg_cadence_rpm: req.avg_cadence_rpm.or(ride.avg_cadence_rpm),
        calories: req.calories.or(ride.calories),
        is_indoor: ride.is_indoor,
    }))
}

async fn list_rides(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<RideResponse>>, (StatusCode, String)> {
    let rides = sqlx::query_as::<_, Ride>(
        "SELECT * FROM rides WHERE user_id = $1 ORDER BY started_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<RideResponse> = rides
        .into_iter()
        .map(|r| RideResponse {
            id: r.id,
            route_id: r.route_id.unwrap_or_default(),
            started_at: r.started_at,
            completed_at: r.completed_at,
            duration_secs: r.duration_secs,
            avg_power_watts: r.avg_power_watts,
            avg_speed_kmh: r.avg_speed_kmh,
            avg_cadence_rpm: r.avg_cadence_rpm,
            calories: r.calories,
            is_indoor: r.is_indoor,
        })
        .collect();

    Ok(Json(responses))
}
