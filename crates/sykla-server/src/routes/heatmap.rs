use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::{DetectedCorridorRecord, HeatmapCellRecord};
use crate::AppState;

pub fn heatmap_router() -> Router<AppState> {
    Router::new()
        .route("/tiles/{zoom}/{x}/{y}", get(get_tile))
        .route("/bounds", get(get_bounds))
        .route("/corridors", get(list_corridors))
        .route("/detect", post(detect_corridors))
        .route("/corridors/{id}/promote", post(promote_corridor))
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BoundsQuery {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

#[derive(Serialize)]
pub struct HeatmapCellResponse {
    pub grid_x: i32,
    pub grid_y: i32,
    pub lat: f64,
    pub lng: f64,
    pub ride_count: i32,
    pub unique_riders: i32,
    pub total_points: i32,
}

#[derive(Serialize)]
pub struct CorridorResponse {
    pub id: Uuid,
    pub popularity_score: f32,
    pub is_loop: bool,
    pub distance_m: Option<f32>,
    pub polyline: Vec<(f64, f64)>,
    pub promoted_route_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct PromoteResponse {
    pub route_id: Uuid,
    pub corridor_id: Uuid,
}

// ---------------------------------------------------------------------------
// GET /api/heatmap/tiles/{zoom}/{x}/{y} — Get heat cells within a tile region
// ---------------------------------------------------------------------------

async fn get_tile(
    State(state): State<AppState>,
    Path((zoom, x, y)): Path<(i32, i32, i32)>,
) -> Result<Json<Vec<HeatmapCellResponse>>, (StatusCode, String)> {
    // Return cells near this tile (the tile itself + neighbors for smooth rendering)
    let cells = sqlx::query_as::<_, HeatmapCellRecord>(
        r#"SELECT zoom, grid_x, grid_y, ride_count, unique_riders, total_points
           FROM heatmap_cells
           WHERE zoom = $1 AND grid_x BETWEEN $2 - 1 AND $2 + 1
             AND grid_y BETWEEN $3 - 1 AND $3 + 1
             AND ride_count > 0"#,
    )
    .bind(zoom)
    .bind(x)
    .bind(y)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<HeatmapCellResponse> = cells
        .into_iter()
        .map(|c| {
            let (lat, lng) = sykla_core::heatmap::grid_to_lat_lng(c.grid_x, c.grid_y, zoom as u32);
            HeatmapCellResponse {
                grid_x: c.grid_x,
                grid_y: c.grid_y,
                lat,
                lng,
                ride_count: c.ride_count,
                unique_riders: c.unique_riders,
                total_points: c.total_points,
            }
        })
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// GET /api/heatmap/bounds — Get heat cells within a bounding box
// ---------------------------------------------------------------------------

async fn get_bounds(
    State(state): State<AppState>,
    Query(bounds): Query<BoundsQuery>,
) -> Result<Json<Vec<HeatmapCellResponse>>, (StatusCode, String)> {
    let zoom = 15u32;

    // Convert bounding box to grid coordinates
    let (min_x, min_y) = sykla_core::heatmap::lat_lng_to_grid(bounds.north, bounds.west, zoom);
    let (max_x, max_y) = sykla_core::heatmap::lat_lng_to_grid(bounds.south, bounds.east, zoom);

    let cells = sqlx::query_as::<_, HeatmapCellRecord>(
        r#"SELECT zoom, grid_x, grid_y, ride_count, unique_riders, total_points
           FROM heatmap_cells
           WHERE zoom = $1
             AND grid_x BETWEEN $2 AND $3
             AND grid_y BETWEEN $4 AND $5
             AND ride_count > 0"#,
    )
    .bind(zoom as i32)
    .bind(min_x)
    .bind(max_x)
    .bind(min_y)
    .bind(max_y)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<HeatmapCellResponse> = cells
        .into_iter()
        .map(|c| {
            let (lat, lng) = sykla_core::heatmap::grid_to_lat_lng(c.grid_x, c.grid_y, zoom);
            HeatmapCellResponse {
                grid_x: c.grid_x,
                grid_y: c.grid_y,
                lat,
                lng,
                ride_count: c.ride_count,
                unique_riders: c.unique_riders,
                total_points: c.total_points,
            }
        })
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// GET /api/heatmap/corridors — List detected popular corridors
// ---------------------------------------------------------------------------

async fn list_corridors(
    State(state): State<AppState>,
) -> Result<Json<Vec<CorridorResponse>>, (StatusCode, String)> {
    let corridors = sqlx::query_as::<_, DetectedCorridorRecord>(
        r#"SELECT id, popularity_score, is_loop, distance_m, polyline_json,
            promoted_route_id, created_at
           FROM detected_corridors ORDER BY popularity_score DESC LIMIT 100"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<CorridorResponse> = corridors
        .into_iter()
        .map(|c| {
            let polyline: Vec<(f64, f64)> =
                serde_json::from_str(&c.polyline_json).unwrap_or_default();
            CorridorResponse {
                id: c.id,
                popularity_score: c.popularity_score,
                is_loop: c.is_loop,
                distance_m: c.distance_m,
                polyline,
                promoted_route_id: c.promoted_route_id,
            }
        })
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// POST /api/heatmap/detect — Trigger corridor detection
// ---------------------------------------------------------------------------

async fn detect_corridors(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<CorridorResponse>>), (StatusCode, String)> {
    // Fetch all heatmap cells
    let cells_rows = sqlx::query_as::<_, HeatmapCellRecord>(
        "SELECT zoom, grid_x, grid_y, ride_count, unique_riders, total_points FROM heatmap_cells WHERE ride_count > 0",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut cells: HashMap<(i32, i32), u32> = HashMap::new();
    for c in &cells_rows {
        cells.insert((c.grid_x, c.grid_y), c.ride_count as u32);
    }

    let config = sykla_core::corridor_detector::CorridorConfig::default();
    let detected = sykla_core::corridor_detector::detect_corridors(&cells, &config);

    // Clear old detections and insert new ones
    sqlx::query("DELETE FROM detected_corridors WHERE promoted_route_id IS NULL")
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response = Vec::new();

    for corridor in &detected {
        let polyline_json = serde_json::to_string(&corridor.points)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO detected_corridors (popularity_score, is_loop, distance_m, polyline_json)
               VALUES ($1, $2, $3, $4) RETURNING id"#,
        )
        .bind(corridor.popularity_score as f32)
        .bind(corridor.is_loop)
        .bind(corridor.distance_m as f32)
        .bind(&polyline_json)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        response.push(CorridorResponse {
            id,
            popularity_score: corridor.popularity_score as f32,
            is_loop: corridor.is_loop,
            distance_m: Some(corridor.distance_m as f32),
            polyline: corridor.points.clone(),
            promoted_route_id: None,
        });
    }

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// POST /api/heatmap/corridors/{id}/promote — Convert corridor into a Route
// ---------------------------------------------------------------------------

async fn promote_corridor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<PromoteResponse>), (StatusCode, String)> {
    let corridor = sqlx::query_as::<_, DetectedCorridorRecord>(
        "SELECT * FROM detected_corridors WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Corridor not found".to_string()))?;

    if corridor.promoted_route_id.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "Corridor already promoted to a route".to_string(),
        ));
    }

    let polyline: Vec<(f64, f64)> =
        serde_json::from_str(&corridor.polyline_json).unwrap_or_default();

    if polyline.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Corridor has insufficient points".to_string(),
        ));
    }

    // Build RoutePoints from corridor polyline
    // Elevation is interpolated from nearby activity data
    let mut route_points = Vec::with_capacity(polyline.len());
    let mut cumulative_distance = 0.0_f64;

    for (i, &(lat, lng)) in polyline.iter().enumerate() {
        if i > 0 {
            let (prev_lat, prev_lng) = polyline[i - 1];
            cumulative_distance += sykla_core::geo_math::haversine_distance(prev_lat, prev_lng, lat, lng);
        }

        // Query nearest activity point for elevation
        let elevation: f64 = sqlx::query_scalar::<_, Option<f32>>(
            r#"SELECT elevation_m FROM activity_points
               WHERE elevation_m IS NOT NULL
               ORDER BY (lat - $1) * (lat - $1) + (lng - $2) * (lng - $2) ASC
               LIMIT 1"#,
        )
        .bind(lat)
        .bind(lng)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .flatten()
        .unwrap_or(0.0) as f64;

        route_points.push(sykla_core::types::RoutePoint {
            lat,
            lng,
            elevation_m: elevation,
            distance_from_start_m: cumulative_distance,
            grade_percent: None,
            surface: Default::default(),
        });
    }

    sykla_core::gradient::calculate_grades(&mut route_points, 5);

    let total_distance = route_points
        .last()
        .map(|p| p.distance_from_start_m)
        .unwrap_or(0.0);

    let mut elevation_gain = 0.0_f64;
    let mut min_elev = f64::MAX;
    let mut max_elev = f64::MIN;
    for (i, p) in route_points.iter().enumerate() {
        min_elev = min_elev.min(p.elevation_m);
        max_elev = max_elev.max(p.elevation_m);
        if i > 0 {
            let diff = p.elevation_m - route_points[i - 1].elevation_m;
            if diff > 0.0 {
                elevation_gain += diff;
            }
        }
    }

    let start_pos = route_points.first().map(|p| (p.lat, p.lng));

    let route_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO routes (name, description, gpx_data, distance_m, elevation_gain_m,
            min_elevation_m, max_elevation_m, start_lat, start_lng, is_public, source)
           VALUES ($1, $2, '', $3, $4, $5, $6, $7, $8, true, 'corridor') RETURNING id"#,
    )
    .bind(format!(
        "Popular {} ({:.1} km)",
        if corridor.is_loop { "Loop" } else { "Route" },
        total_distance / 1000.0
    ))
    .bind("Auto-generated from heat map corridor detection")
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

    // Insert route points
    for (i, point) in route_points.iter().enumerate() {
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

    // Link corridor to route
    sqlx::query("UPDATE detected_corridors SET promoted_route_id = $1 WHERE id = $2")
        .bind(route_id)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(PromoteResponse {
            route_id,
            corridor_id: id,
        }),
    ))
}
