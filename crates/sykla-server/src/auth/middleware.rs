use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub exp: usize,
}

/// Extract user claims from the request extensions (set by middleware).
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
}

pub async fn auth_middleware(mut req: Request, next: Next) -> Response {
    let jwt_secret = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = if let Some(token) = auth_header.strip_prefix("Bearer ") {
        token
    } else {
        return (StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header").into_response();
    };

    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    let auth_user = AuthUser {
        user_id: token_data.claims.sub,
        email: token_data.claims.email,
    };

    req.extensions_mut().insert(auth_user);
    next.run(req).await
}
