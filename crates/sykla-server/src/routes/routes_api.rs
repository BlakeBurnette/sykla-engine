use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::models::{RoutePointRecord, RouteSummary};
use crate::AppState;

pub fn routes_router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_route).get(list_routes))
        .route("/{id}", get(get_route).delete(delete_route))
}

#[derive(Deserialize)]
pub struct CreateRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub gpx_data: String,
    pub city: Option<String>,
    pub is_public: Option<bool>,
    pub source: Option<String>,
}

#[derive(Serialize)]
pub struct RouteDetailResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub distance_m: f32,
    pub elevation_gain_m: f32,
    pub min_elevation_m: Option<f32>,
    pub max_elevation_m: Option<f32>,
    pub start_lat: Option<f64>,
    pub start_lng: Option<f64>,
    pub city: Option<String>,
    pub is_public: Option<bool>,
    pub source: Option<String>,
    pub points: Vec<RoutePointRecord>,
}

async fn create_route(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(req): Json<CreateRouteRequest>,
) -> Result<(StatusCode, Json<RouteDetailResponse>), (StatusCode, String)> {
    // Parse GPX
    let route = sykla_core::gpx_parser::parse_gpx(&req.gpx_data)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid GPX: {e}")))?;

    let name = req.name.unwrap_or(route.name.clone());
    let start_pos = route.start_position();

    // Insert route record
    let route_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO routes (user_id, name, description, gpx_data, distance_m, elevation_gain_m,
            min_elevation_m, max_elevation_m, start_lat, start_lng, city, is_public, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id"#,
    )
    .bind(auth.user_id)
    .bind(&name)
    .bind(&req.description)
    .bind(&req.gpx_data)
    .bind(route.total_distance_m as f32)
    .bind(route.elevation_gain_m as f32)
    .bind(Some(route.min_elevation_m as f32))
    .bind(Some(route.max_elevation_m as f32))
    .bind(start_pos.map(|p| p.0))
    .bind(start_pos.map(|p| p.1))
    .bind(&req.city)
    .bind(req.is_public.unwrap_or(false))
    .bind(req.source.as_deref().unwrap_or("gpx_upload"))
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Insert route points
    for (i, point) in route.points.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO route_points (route_id, seq, lat, lng, elevation_m, distance_from_start_m, grade_percent)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(route_id)
        .bind(i as i32)
        .bind(point.lat)
        .bind(point.lng)
        .bind(point.elevation_m as f32)
        .bind(point.distance_from_start_m as f32)
        .bind(point.grade_percent.map(|g| g as f32))
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Fetch back the points
    let points = sqlx::query_as::<_, RoutePointRecord>(
        "SELECT id, route_id, seq, lat, lng, elevation_m, distance_from_start_m, grade_percent FROM route_points WHERE route_id = $1 ORDER BY seq",
    )
    .bind(route_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(RouteDetailResponse {
            id: route_id,
            name,
            description: req.description,
            distance_m: route.total_distance_m as f32,
            elevation_gain_m: route.elevation_gain_m as f32,
            min_elevation_m: Some(route.min_elevation_m as f32),
            max_elevation_m: Some(route.max_elevation_m as f32),
            start_lat: start_pos.map(|p| p.0),
            start_lng: start_pos.map(|p| p.1),
            city: req.city,
            is_public: Some(req.is_public.unwrap_or(false)),
            source: Some(req.source.unwrap_or("gpx_upload".to_string())),
            points,
        }),
    ))
}

async fn list_routes(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<RouteSummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, f32, f32, Option<String>, Option<bool>, Option<String>, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT id, name, description, distance_m, elevation_gain_m, city, is_public, source, created_at
           FROM routes WHERE user_id = $1 OR is_public = true ORDER BY created_at DESC"#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let summaries = rows
        .into_iter()
        .map(|r| RouteSummary {
            id: r.0,
            name: r.1,
            description: r.2,
            distance_m: r.3,
            elevation_gain_m: r.4,
            city: r.5,
            is_public: r.6,
            source: r.7,
            created_at: r.8,
        })
        .collect();

    Ok(Json(summaries))
}

async fn get_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RouteDetailResponse>, (StatusCode, String)> {
    let route = sqlx::query_as::<_, (Uuid, String, Option<String>, f32, f32, Option<f32>, Option<f32>, Option<f64>, Option<f64>, Option<String>, Option<bool>, Option<String>)>(
        r#"SELECT id, name, description, distance_m, elevation_gain_m, min_elevation_m, max_elevation_m,
            start_lat, start_lng, city, is_public, source
           FROM routes WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let route = route.ok_or((StatusCode::NOT_FOUND, "Route not found".to_string()))?;

    let points = sqlx::query_as::<_, RoutePointRecord>(
        "SELECT id, route_id, seq, lat, lng, elevation_m, distance_from_start_m, grade_percent FROM route_points WHERE route_id = $1 ORDER BY seq",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RouteDetailResponse {
        id: route.0,
        name: route.1,
        description: route.2,
        distance_m: route.3,
        elevation_gain_m: route.4,
        min_elevation_m: route.5,
        max_elevation_m: route.6,
        start_lat: route.7,
        start_lng: route.8,
        city: route.9,
        is_public: route.10,
        source: route.11,
        points,
    }))
}

async fn delete_route(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query("DELETE FROM routes WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Route not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
