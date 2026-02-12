use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};

use crate::models::{CityRecord, CitySummary};
use crate::AppState;

pub fn cities_router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list_cities))
        .route("/{slug}", axum::routing::get(get_city))
}

fn to_summary(c: CityRecord) -> CitySummary {
    CitySummary {
        slug: c.slug,
        name: c.name,
        center_lat: c.center_lat,
        center_lng: c.center_lng,
        file_size_bytes: c.file_size_bytes,
        download_url: c.download_url,
        version: c.version,
    }
}

async fn list_cities(
    State(state): State<AppState>,
) -> Result<Json<Vec<CitySummary>>, (StatusCode, String)> {
    let cities = sqlx::query_as::<_, CityRecord>(
        "SELECT * FROM cities ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(cities.into_iter().map(to_summary).collect()))
}

async fn get_city(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<CitySummary>, (StatusCode, String)> {
    let city = sqlx::query_as::<_, CityRecord>(
        "SELECT * FROM cities WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, format!("City '{slug}' not found")))?;

    Ok(Json(to_summary(city)))
}
