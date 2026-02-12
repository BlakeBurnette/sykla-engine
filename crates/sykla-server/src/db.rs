use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::time::Duration;

/// Parse DATABASE_URL and handle Cloud SQL socket connections.
/// For Cloud SQL, set DATABASE_URL to a standard postgres:// URL and
/// CLOUD_SQL_CONNECTION_NAME to the instance connection name.
fn build_connect_options(database_url: &str) -> Result<PgConnectOptions, sqlx::Error> {
    // Check if there's a ?host= param (Cloud SQL socket path) and handle it
    if let Some(socket_path) = std::env::var("CLOUD_SQL_SOCKET_DIR").ok() {
        let conn_name = std::env::var("CLOUD_SQL_CONNECTION_NAME")
            .unwrap_or_default();
        let socket_dir = if conn_name.is_empty() {
            socket_path
        } else {
            format!("{}/{}", socket_path, conn_name)
        };
        let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "sykla".to_string());
        let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
        let db_pass = std::env::var("DB_PASS").unwrap_or_default();

        tracing::info!("Using Cloud SQL socket: {}", socket_dir);
        let opts = PgConnectOptions::new()
            .socket(&socket_dir)
            .username(&db_user)
            .password(&db_pass)
            .database(&db_name);
        Ok(opts)
    } else {
        // Standard TCP connection via DATABASE_URL
        database_url.parse::<PgConnectOptions>()
    }
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options = build_connect_options(database_url)?;

    // Retry connection up to 5 times (Cloud SQL proxy may need a moment to start)
    let mut last_err = None;
    for attempt in 1..=5 {
        tracing::info!("Database connection attempt {}/5...", attempt);
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options.clone())
            .await
        {
            Ok(pool) => {
                tracing::info!("Database connected, running migrations...");
                sqlx::migrate!("../../migrations").run(&pool).await?;
                tracing::info!("Migrations complete");
                return Ok(pool);
            }
            Err(e) => {
                tracing::warn!("Connection attempt {} failed: {}", attempt, e);
                last_err = Some(e);
                if attempt < 5 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}
