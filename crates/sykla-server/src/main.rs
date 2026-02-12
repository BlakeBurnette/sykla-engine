mod auth;
mod config;
mod db;
mod models;
mod routes;
mod ws;

use axum::{middleware, routing::get, Router};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use auth::{auth_middleware, auth_router};
use config::Config;
use routes::{rides::rides_router, routes_api::routes_router, users::users_router};
use ws::{new_room_manager, ws_handler, RoomManager};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub rooms: RoomManager,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sykla_server=debug,tower_http=debug".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!("Starting Sykla server on {}", config.bind_addr());
    tracing::info!("DATABASE_URL host: {}", config.database_url.split('@').last().unwrap_or("unknown"));

    let db = match db::create_pool(&config.database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    let state = AppState {
        db,
        jwt_secret: config.jwt_secret.clone(),
        rooms: new_room_manager(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth)
    let public_routes = Router::new()
        .nest("/api/auth", auth_router())
        .route("/api/sync/{route_id}", get(ws_handler));

    // Protected routes (require JWT)
    let protected_routes = Router::new()
        .nest("/api/routes", routes_router())
        .nest("/api/rides", rides_router())
        .nest("/api/users", users_router())
        .layer(middleware::from_fn(auth_middleware));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors)
        .layer(axum::Extension(config.jwt_secret.clone()))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr())
        .await
        .expect("Failed to bind");

    tracing::info!("Sykla server listening on {}", config.bind_addr());
    axum::serve(listener, app).await.expect("Server error");
}
