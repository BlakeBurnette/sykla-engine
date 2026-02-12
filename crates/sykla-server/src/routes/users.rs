use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::AppState;

pub fn users_router() -> Router<AppState> {
    Router::new().route("/me", get(get_me).put(update_me))
}

#[derive(Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub weight_kg: Option<f32>,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub weight_kg: Option<f32>,
}

async fn get_me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<UserProfile>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (Uuid, String, String, Option<f32>)>(
        "SELECT id, email, display_name, weight_kg FROM users WHERE id = $1",
    )
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(UserProfile {
        id: row.0,
        email: row.1,
        display_name: row.2,
        weight_kg: row.3,
    }))
}

async fn update_me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UserProfile>, (StatusCode, String)> {
    sqlx::query(
        r#"UPDATE users SET
           display_name = COALESCE($1, display_name),
           weight_kg = COALESCE($2, weight_kg)
           WHERE id = $3"#,
    )
    .bind(&req.display_name)
    .bind(req.weight_kg)
    .bind(auth.user_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Re-fetch
    get_me(State(state), Extension(auth)).await
}
