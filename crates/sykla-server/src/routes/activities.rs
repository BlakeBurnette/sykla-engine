use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::models::{ActivityPointRecord, ActivitySummary};
use crate::AppState;

pub fn activities_router() -> Router<AppState> {
    Router::new()
        .route("/", post(start_activity).get(list_activities))
        .route("/import", post(import_gpx_activity))
        .route(
            "/{id}",
            get(get_activity).put(complete_activity).delete(delete_activity),
        )
        .route("/{id}/points", post(upload_points))
        .route("/{id}/to-route", post(activity_to_route))
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct StartActivityRequest {
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct ImportGpxRequest {
    pub name: Option<String>,
    pub gpx_data: String,
}

#[derive(Deserialize)]
pub struct UploadPointsRequest {
    pub points: Vec<PointData>,
}

#[derive(Deserialize)]
pub struct PointData {
    pub seq: i32,
    pub timestamp_ms: i64,
    pub lat: f64,
    pub lng: f64,
    pub elevation_m: Option<f64>,
    pub speed_kmh: Option<f64>,
    pub power_watts: Option<f64>,
    pub heart_rate_bpm: Option<u8>,
    pub cadence_rpm: Option<f64>,
    pub distance_from_start_m: f64,
    pub grade_percent: Option<f64>,
}

#[derive(Deserialize)]
pub struct CompleteActivityRequest {
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct ActivityResponse {
    pub id: Uuid,
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
    pub generated_route_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct ActivityDetailResponse {
    pub id: Uuid,
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
    pub generated_route_id: Option<Uuid>,
    pub points: Vec<ActivityPointRecord>,
}

#[derive(Serialize)]
pub struct RouteConversionResponse {
    pub route_id: Uuid,
    pub activity_id: Uuid,
}

// ---------------------------------------------------------------------------
// POST /api/activities — Start recording session
// ---------------------------------------------------------------------------

async fn start_activity(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(req): Json<StartActivityRequest>,
) -> Result<(StatusCode, Json<ActivityResponse>), (StatusCode, String)> {
    let now = Utc::now();
    let name = req.name.unwrap_or_else(|| "Untitled Activity".to_string());

    let activity_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO activities (user_id, name, source, started_at)
           VALUES ($1, $2, 'recorded', $3) RETURNING id"#,
    )
    .bind(auth.user_id)
    .bind(&name)
    .bind(now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(ActivityResponse {
            id: activity_id,
            name,
            source: "recorded".to_string(),
            started_at: Some(now),
            completed_at: None,
            duration_secs: None,
            distance_m: None,
            elevation_gain_m: None,
            avg_power_watts: None,
            max_power_watts: None,
            avg_speed_kmh: None,
            max_speed_kmh: None,
            avg_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            avg_cadence_rpm: None,
            max_cadence_rpm: None,
            calories: None,
            generated_route_id: None,
        }),
    ))
}

// ---------------------------------------------------------------------------
// POST /api/activities/import — Import GPX file as activity
// ---------------------------------------------------------------------------

async fn import_gpx_activity(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(req): Json<ImportGpxRequest>,
) -> Result<(StatusCode, Json<ActivityResponse>), (StatusCode, String)> {
    // Parse GPX into activity
    let activity = sykla_core::gpx_activity_parser::parse_gpx_activity(&req.gpx_data)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid GPX: {e}")))?;

    let name = req.name.unwrap_or(activity.name.clone());

    // Insert activity record
    let activity_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO activities (
            user_id, name, source, started_at, completed_at,
            duration_secs, distance_m, elevation_gain_m,
            avg_power_watts, max_power_watts, avg_speed_kmh, max_speed_kmh,
            avg_heart_rate_bpm, max_heart_rate_bpm, avg_cadence_rpm, max_cadence_rpm,
            bbox_south, bbox_west, bbox_north, bbox_east, gpx_data
        ) VALUES (
            $1, $2, 'gpx_import', $3, $4,
            $5, $6, $7,
            $8, $9, $10, $11,
            $12, $13, $14, $15,
            $16, $17, $18, $19, $20
        ) RETURNING id"#,
    )
    .bind(auth.user_id)
    .bind(&name)
    .bind(activity.started_at)
    .bind(activity.started_at.map(|s| {
        s + chrono::Duration::milliseconds((activity.duration_secs * 1000.0) as i64)
    }))
    .bind(activity.duration_secs as f32)
    .bind(activity.total_distance_m as f32)
    .bind(activity.elevation_gain_m as f32)
    .bind(activity.avg_power_watts.map(|v| v as f32))
    .bind(activity.max_power_watts.map(|v| v as f32))
    .bind(activity.avg_speed_kmh.map(|v| v as f32))
    .bind(activity.max_speed_kmh.map(|v| v as f32))
    .bind(activity.avg_heart_rate_bpm.map(|v| v as i16))
    .bind(activity.max_heart_rate_bpm.map(|v| v as i16))
    .bind(activity.avg_cadence_rpm.map(|v| v as f32))
    .bind(activity.max_cadence_rpm.map(|v| v as f32))
    .bind(activity.bbox.map(|b| b.south))
    .bind(activity.bbox.map(|b| b.west))
    .bind(activity.bbox.map(|b| b.north))
    .bind(activity.bbox.map(|b| b.east))
    .bind(&req.gpx_data)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Batch insert activity points using multi-row INSERT
    if !activity.points.is_empty() {
        insert_activity_points_batch(&state.db, activity_id, &activity.points).await?;
    }

    // Update heatmap
    update_heatmap_for_activity(&state.db, activity_id, auth.user_id, &activity.points).await?;

    let completed_at = activity.started_at.map(|s| {
        s + chrono::Duration::milliseconds((activity.duration_secs * 1000.0) as i64)
    });

    Ok((
        StatusCode::CREATED,
        Json(ActivityResponse {
            id: activity_id,
            name,
            source: "gpx_import".to_string(),
            started_at: activity.started_at,
            completed_at,
            duration_secs: Some(activity.duration_secs as f32),
            distance_m: Some(activity.total_distance_m as f32),
            elevation_gain_m: Some(activity.elevation_gain_m as f32),
            avg_power_watts: activity.avg_power_watts.map(|v| v as f32),
            max_power_watts: activity.max_power_watts.map(|v| v as f32),
            avg_speed_kmh: activity.avg_speed_kmh.map(|v| v as f32),
            max_speed_kmh: activity.max_speed_kmh.map(|v| v as f32),
            avg_heart_rate_bpm: activity.avg_heart_rate_bpm.map(|v| v as i16),
            max_heart_rate_bpm: activity.max_heart_rate_bpm.map(|v| v as i16),
            avg_cadence_rpm: activity.avg_cadence_rpm.map(|v| v as f32),
            max_cadence_rpm: activity.max_cadence_rpm.map(|v| v as f32),
            calories: activity.calories,
            generated_route_id: None,
        }),
    ))
}

// ---------------------------------------------------------------------------
// POST /api/activities/{id}/points — Batch upload points
// ---------------------------------------------------------------------------

async fn upload_points(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UploadPointsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Verify activity belongs to user
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM activities WHERE id = $1 AND user_id = $2)",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, "Activity not found".to_string()));
    }

    // Convert to ActivityPoints for batch insert
    let points: Vec<sykla_core::activity::ActivityPoint> = req
        .points
        .iter()
        .map(|p| sykla_core::activity::ActivityPoint {
            lat: p.lat,
            lng: p.lng,
            elevation_m: p.elevation_m,
            timestamp_ms: p.timestamp_ms,
            speed_kmh: p.speed_kmh,
            power_watts: p.power_watts,
            heart_rate_bpm: p.heart_rate_bpm,
            cadence_rpm: p.cadence_rpm,
            distance_from_start_m: p.distance_from_start_m,
            grade_percent: p.grade_percent,
        })
        .collect();

    insert_activity_points_batch_with_seq(&state.db, id, &req.points).await?;

    // Update heatmap incrementally
    update_heatmap_for_activity(&state.db, id, auth.user_id, &points).await?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// PUT /api/activities/{id} — Complete activity (server computes aggregates)
// ---------------------------------------------------------------------------

async fn complete_activity(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteActivityRequest>,
) -> Result<Json<ActivityResponse>, (StatusCode, String)> {
    // Verify ownership
    let activity = sqlx::query_as::<_, crate::models::ActivityRecord>(
        "SELECT * FROM activities WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Activity not found".to_string()))?;

    // Fetch all points to compute aggregates
    let points = sqlx::query_as::<_, ActivityPointRecord>(
        "SELECT * FROM activity_points WHERE activity_id = $1 ORDER BY seq",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Compute aggregates from points
    let core_points: Vec<sykla_core::activity::ActivityPoint> = points
        .iter()
        .map(|p| sykla_core::activity::ActivityPoint {
            lat: p.lat,
            lng: p.lng,
            elevation_m: p.elevation_m.map(|v| v as f64),
            timestamp_ms: p.timestamp_ms,
            speed_kmh: p.speed_kmh.map(|v| v as f64),
            power_watts: p.power_watts.map(|v| v as f64),
            heart_rate_bpm: p.heart_rate_bpm.map(|v| v as u8),
            cadence_rpm: p.cadence_rpm.map(|v| v as f64),
            distance_from_start_m: p.distance_from_start_m as f64,
            grade_percent: p.grade_percent.map(|v| v as f64),
        })
        .collect();

    let agg = sykla_core::activity::Activity::compute_aggregates(&core_points);
    let completed_at = req.completed_at.unwrap_or_else(Utc::now);

    sqlx::query(
        r#"UPDATE activities SET
            completed_at = $1, duration_secs = $2, distance_m = $3, elevation_gain_m = $4,
            avg_power_watts = $5, max_power_watts = $6,
            avg_speed_kmh = $7, max_speed_kmh = $8,
            avg_heart_rate_bpm = $9, max_heart_rate_bpm = $10,
            avg_cadence_rpm = $11, max_cadence_rpm = $12,
            bbox_south = $13, bbox_west = $14, bbox_north = $15, bbox_east = $16
           WHERE id = $17"#,
    )
    .bind(completed_at)
    .bind(agg.duration_secs as f32)
    .bind(agg.total_distance_m as f32)
    .bind(agg.elevation_gain_m as f32)
    .bind(agg.avg_power_watts.map(|v| v as f32))
    .bind(agg.max_power_watts.map(|v| v as f32))
    .bind(agg.avg_speed_kmh.map(|v| v as f32))
    .bind(agg.max_speed_kmh.map(|v| v as f32))
    .bind(agg.avg_heart_rate_bpm.map(|v| v as i16))
    .bind(agg.max_heart_rate_bpm.map(|v| v as i16))
    .bind(agg.avg_cadence_rpm.map(|v| v as f32))
    .bind(agg.max_cadence_rpm.map(|v| v as f32))
    .bind(agg.bbox.map(|b| b.south))
    .bind(agg.bbox.map(|b| b.west))
    .bind(agg.bbox.map(|b| b.north))
    .bind(agg.bbox.map(|b| b.east))
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ActivityResponse {
        id,
        name: activity.name,
        source: activity.source,
        started_at: activity.started_at,
        completed_at: Some(completed_at),
        duration_secs: Some(agg.duration_secs as f32),
        distance_m: Some(agg.total_distance_m as f32),
        elevation_gain_m: Some(agg.elevation_gain_m as f32),
        avg_power_watts: agg.avg_power_watts.map(|v| v as f32),
        max_power_watts: agg.max_power_watts.map(|v| v as f32),
        avg_speed_kmh: agg.avg_speed_kmh.map(|v| v as f32),
        max_speed_kmh: agg.max_speed_kmh.map(|v| v as f32),
        avg_heart_rate_bpm: agg.avg_heart_rate_bpm.map(|v| v as i16),
        max_heart_rate_bpm: agg.max_heart_rate_bpm.map(|v| v as i16),
        avg_cadence_rpm: agg.avg_cadence_rpm.map(|v| v as f32),
        max_cadence_rpm: agg.max_cadence_rpm.map(|v| v as f32),
        calories: activity.calories,
        generated_route_id: activity.generated_route_id,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/activities — List user's activities
// ---------------------------------------------------------------------------

async fn list_activities(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<ActivitySummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<i16>, Option<f32>, Option<DateTime<Utc>>)>(
        r#"SELECT id, name, source, started_at, completed_at, duration_secs, distance_m,
            elevation_gain_m, avg_power_watts, avg_speed_kmh, avg_heart_rate_bpm,
            avg_cadence_rpm, created_at
           FROM activities WHERE user_id = $1 ORDER BY started_at DESC"#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let summaries = rows
        .into_iter()
        .map(|r| ActivitySummary {
            id: r.0,
            name: r.1,
            source: r.2,
            started_at: r.3,
            completed_at: r.4,
            duration_secs: r.5,
            distance_m: r.6,
            elevation_gain_m: r.7,
            avg_power_watts: r.8,
            avg_speed_kmh: r.9,
            avg_heart_rate_bpm: r.10,
            avg_cadence_rpm: r.11,
            created_at: r.12,
        })
        .collect();

    Ok(Json(summaries))
}

// ---------------------------------------------------------------------------
// GET /api/activities/{id} — Get activity with all points
// ---------------------------------------------------------------------------

async fn get_activity(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ActivityDetailResponse>, (StatusCode, String)> {
    let activity = sqlx::query_as::<_, crate::models::ActivityRecord>(
        "SELECT * FROM activities WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Activity not found".to_string()))?;

    let points = sqlx::query_as::<_, ActivityPointRecord>(
        r#"SELECT id, activity_id, seq, timestamp_ms, lat, lng, elevation_m,
            speed_kmh, power_watts, heart_rate_bpm, cadence_rpm,
            distance_from_start_m, grade_percent
           FROM activity_points WHERE activity_id = $1 ORDER BY seq"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ActivityDetailResponse {
        id: activity.id,
        name: activity.name,
        source: activity.source,
        started_at: activity.started_at,
        completed_at: activity.completed_at,
        duration_secs: activity.duration_secs,
        distance_m: activity.distance_m,
        elevation_gain_m: activity.elevation_gain_m,
        avg_power_watts: activity.avg_power_watts,
        max_power_watts: activity.max_power_watts,
        avg_speed_kmh: activity.avg_speed_kmh,
        max_speed_kmh: activity.max_speed_kmh,
        avg_heart_rate_bpm: activity.avg_heart_rate_bpm,
        max_heart_rate_bpm: activity.max_heart_rate_bpm,
        avg_cadence_rpm: activity.avg_cadence_rpm,
        max_cadence_rpm: activity.max_cadence_rpm,
        calories: activity.calories,
        generated_route_id: activity.generated_route_id,
        points,
    }))
}

// ---------------------------------------------------------------------------
// DELETE /api/activities/{id} — Delete activity (cascades points, decrements heatmap)
// ---------------------------------------------------------------------------

async fn delete_activity(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Decrement heatmap before deleting (contributions cascade, so read them first)
    let contributions = sqlx::query_as::<_, (i32, i32, i32, i32)>(
        "SELECT zoom, grid_x, grid_y, point_count FROM heatmap_contributions WHERE activity_id = $1",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (zoom, gx, gy, pt_count) in &contributions {
        sqlx::query(
            r#"UPDATE heatmap_cells SET
                ride_count = GREATEST(ride_count - 1, 0),
                total_points = GREATEST(total_points - $1, 0),
                unique_riders = GREATEST(unique_riders - 1, 0)
               WHERE zoom = $2 AND grid_x = $3 AND grid_y = $4"#,
        )
        .bind(pt_count)
        .bind(zoom)
        .bind(gx)
        .bind(gy)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Delete activity (cascades points and contributions)
    let result = sqlx::query("DELETE FROM activities WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Activity not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// POST /api/activities/{id}/to-route — Convert activity into a Route
// ---------------------------------------------------------------------------

async fn activity_to_route(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<RouteConversionResponse>), (StatusCode, String)> {
    // Verify ownership
    let activity = sqlx::query_as::<_, crate::models::ActivityRecord>(
        "SELECT * FROM activities WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Activity not found".to_string()))?;

    // Fetch all activity points
    let points = sqlx::query_as::<_, ActivityPointRecord>(
        "SELECT * FROM activity_points WHERE activity_id = $1 ORDER BY seq",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if points.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Activity has no points".to_string(),
        ));
    }

    // Convert to core RoutePoints
    let mut route_points: Vec<sykla_core::types::RoutePoint> = points
        .iter()
        .map(|p| sykla_core::types::RoutePoint {
            lat: p.lat,
            lng: p.lng,
            elevation_m: p.elevation_m.unwrap_or(0.0) as f64,
            distance_from_start_m: p.distance_from_start_m as f64,
            grade_percent: p.grade_percent.map(|g| g as f64),
            surface: Default::default(),
        })
        .collect();

    // Recalculate grades for smoother riding
    sykla_core::gradient::calculate_grades(&mut route_points, 5);

    let start_pos = route_points.first().map(|p| (p.lat, p.lng));
    let total_distance = route_points
        .last()
        .map(|p| p.distance_from_start_m)
        .unwrap_or(0.0);

    // Compute elevation stats
    let mut elevation_gain = 0.0_f64;
    let mut min_elev = f64::MAX;
    let mut max_elev = f64::MIN;
    for (i, p) in route_points.iter().enumerate() {
        if p.elevation_m < min_elev {
            min_elev = p.elevation_m;
        }
        if p.elevation_m > max_elev {
            max_elev = p.elevation_m;
        }
        if i > 0 {
            let diff = p.elevation_m - route_points[i - 1].elevation_m;
            if diff > 0.0 {
                elevation_gain += diff;
            }
        }
    }

    // Build empty GPX data string (route was from activity, not GPX)
    let gpx_data = activity.gpx_data.unwrap_or_default();

    // Insert route record
    let route_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO routes (user_id, name, description, gpx_data, distance_m, elevation_gain_m,
            min_elevation_m, max_elevation_m, start_lat, start_lng, is_public, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, false, 'activity') RETURNING id"#,
    )
    .bind(auth.user_id)
    .bind(&activity.name)
    .bind(format!("Converted from activity {}", id))
    .bind(&gpx_data)
    .bind(total_distance as f32)
    .bind(elevation_gain as f32)
    .bind(if min_elev == f64::MAX {
        None
    } else {
        Some(min_elev as f32)
    })
    .bind(if max_elev == f64::MIN {
        None
    } else {
        Some(max_elev as f32)
    })
    .bind(start_pos.map(|p| p.0))
    .bind(start_pos.map(|p| p.1))
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Batch insert route points
    insert_route_points_batch(&state.db, route_id, &route_points).await?;

    // Link activity to generated route
    sqlx::query("UPDATE activities SET generated_route_id = $1 WHERE id = $2")
        .bind(route_id)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(RouteConversionResponse {
            route_id,
            activity_id: id,
        }),
    ))
}

// ---------------------------------------------------------------------------
// Helper: Batch insert activity points (multi-row INSERT)
// ---------------------------------------------------------------------------

async fn insert_activity_points_batch(
    db: &sqlx::PgPool,
    activity_id: Uuid,
    points: &[sykla_core::activity::ActivityPoint],
) -> Result<(), (StatusCode, String)> {
    // Insert in chunks of 500 to stay within Postgres parameter limits
    for chunk in points.chunks(500) {
        let mut query = String::from(
            "INSERT INTO activity_points (activity_id, seq, timestamp_ms, lat, lng, elevation_m, speed_kmh, power_watts, heart_rate_bpm, cadence_rpm, distance_from_start_m, grade_percent) VALUES ",
        );

        let mut args = sqlx::postgres::PgArguments::default();
        let mut param_idx = 1;

        for (i, point) in chunk.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                param_idx,
                param_idx + 1,
                param_idx + 2,
                param_idx + 3,
                param_idx + 4,
                param_idx + 5,
                param_idx + 6,
                param_idx + 7,
                param_idx + 8,
                param_idx + 9,
                param_idx + 10,
                param_idx + 11,
            ));
            param_idx += 12;

            use sqlx::Arguments;
            args.add(activity_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(i as i32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.timestamp_ms).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lat).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lng).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.elevation_m.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.speed_kmh.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.power_watts.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.heart_rate_bpm.map(|v| v as i16)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.cadence_rpm.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.distance_from_start_m as f32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.grade_percent.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        query.push_str(" ON CONFLICT (activity_id, seq) DO NOTHING");

        sqlx::query_with(&query, args)
            .execute(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(())
}

/// Batch insert with explicit seq values from the request.
async fn insert_activity_points_batch_with_seq(
    db: &sqlx::PgPool,
    activity_id: Uuid,
    points: &[PointData],
) -> Result<(), (StatusCode, String)> {
    for chunk in points.chunks(500) {
        let mut query = String::from(
            "INSERT INTO activity_points (activity_id, seq, timestamp_ms, lat, lng, elevation_m, speed_kmh, power_watts, heart_rate_bpm, cadence_rpm, distance_from_start_m, grade_percent) VALUES ",
        );

        let mut args = sqlx::postgres::PgArguments::default();
        let mut param_idx = 1;

        for (i, point) in chunk.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                param_idx,
                param_idx + 1,
                param_idx + 2,
                param_idx + 3,
                param_idx + 4,
                param_idx + 5,
                param_idx + 6,
                param_idx + 7,
                param_idx + 8,
                param_idx + 9,
                param_idx + 10,
                param_idx + 11,
            ));
            param_idx += 12;

            use sqlx::Arguments;
            args.add(activity_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.seq).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.timestamp_ms).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lat).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lng).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.elevation_m.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.speed_kmh.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.power_watts.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.heart_rate_bpm.map(|v| v as i16)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.cadence_rpm.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.distance_from_start_m as f32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.grade_percent.map(|v| v as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        query.push_str(" ON CONFLICT (activity_id, seq) DO NOTHING");

        sqlx::query_with(&query, args)
            .execute(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(())
}

/// Batch insert route points (for activity-to-route conversion).
async fn insert_route_points_batch(
    db: &sqlx::PgPool,
    route_id: Uuid,
    points: &[sykla_core::types::RoutePoint],
) -> Result<(), (StatusCode, String)> {
    for chunk in points.chunks(500) {
        let mut query = String::from(
            "INSERT INTO route_points (route_id, seq, lat, lng, elevation_m, distance_from_start_m, grade_percent) VALUES ",
        );

        let mut args = sqlx::postgres::PgArguments::default();
        let mut param_idx = 1;

        for (i, point) in chunk.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                param_idx,
                param_idx + 1,
                param_idx + 2,
                param_idx + 3,
                param_idx + 4,
                param_idx + 5,
                param_idx + 6,
            ));
            param_idx += 7;

            use sqlx::Arguments;
            args.add(route_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(i as i32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lat).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.lng).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.elevation_m as f32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.distance_from_start_m as f32).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            args.add(point.grade_percent.map(|g| g as f32)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        sqlx::query_with(&query, args)
            .execute(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: Update heatmap for a set of activity points
// ---------------------------------------------------------------------------

pub async fn update_heatmap_for_activity(
    db: &sqlx::PgPool,
    activity_id: Uuid,
    user_id: Uuid,
    points: &[sykla_core::activity::ActivityPoint],
) -> Result<(), (StatusCode, String)> {
    if points.is_empty() {
        return Ok(());
    }

    let lat_lngs: Vec<(f64, f64)> = points.iter().map(|p| (p.lat, p.lng)).collect();
    let zoom = 15u32;
    let cells = sykla_core::heatmap::rasterize_track(&lat_lngs, zoom);

    for &(gx, gy) in &cells {
        // UPSERT heatmap cell
        sqlx::query(
            r#"INSERT INTO heatmap_cells (zoom, grid_x, grid_y, ride_count, unique_riders, total_points)
               VALUES ($1, $2, $3, 1, 1, $4)
               ON CONFLICT (zoom, grid_x, grid_y) DO UPDATE SET
                 ride_count = heatmap_cells.ride_count + 1,
                 unique_riders = heatmap_cells.unique_riders + 1,
                 total_points = heatmap_cells.total_points + $4,
                 last_updated = now()"#,
        )
        .bind(zoom as i32)
        .bind(gx)
        .bind(gy)
        .bind(1i32)
        .execute(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Record contribution for undo
        sqlx::query(
            r#"INSERT INTO heatmap_contributions (activity_id, zoom, grid_x, grid_y, point_count)
               VALUES ($1, $2, $3, $4, 1)
               ON CONFLICT (activity_id, zoom, grid_x, grid_y) DO UPDATE SET
                 point_count = heatmap_contributions.point_count + 1"#,
        )
        .bind(activity_id)
        .bind(zoom as i32)
        .bind(gx)
        .bind(gy)
        .execute(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    let _ = user_id; // used for unique_riders in a more sophisticated impl

    Ok(())
}
