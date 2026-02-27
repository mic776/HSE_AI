use quiz_backend::{build_state, routes::build_router};
use sqlx::mysql::MySqlPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let state = build_state()?;
    let app = build_router(state);

    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        if !db_url.trim().is_empty() {
            match MySqlPoolOptions::new().max_connections(5).connect(&db_url).await {
                Ok(pool) => match sqlx::migrate!("./migrations").run(&pool).await {
                    Ok(_) => tracing::info!("mysql connected and migrations applied"),
                    Err(err) => tracing::warn!("mysql connected but migrations failed: {}", err),
                },
                Err(err) => {
                    tracing::warn!(
                        "mysql is unavailable ({}), backend continues in local in-memory mode",
                        err
                    );
                }
            }
        }
    }

    let host = std::env::var("BACKEND_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("BACKEND_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("backend listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
