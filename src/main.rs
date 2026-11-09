use axum::Router;
use axum::response::IntoResponse;
use bibby::infra::db::Database;
use bibby::infra::{api, assets};
use bibby::{AppError, AppState};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();

    let db = Arc::new(Database::init().await);
    let state = Arc::new(AppState::new(db));

    let serve_static = Router::new()
        .nest_service("/assets", ServeDir::new("public"))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            assets::cache_control,
        ));

    let pool_for_shutdown = state.db.clone();
    let app = api::routes()
        .merge(serve_static)
        .fallback(not_found_handler)
        .with_state(state)
        .layer(CompressionLayer::new())
        .into_make_service_with_connect_info::<SocketAddr>();

    tracing::info!("listening on http://localhost:{}", port);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Failed to start.");

    tracing::info!("Closing database pool.");
    pool_for_shutdown.close().await;
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, draining connections.");
}

async fn not_found_handler() -> impl IntoResponse {
    AppError::NotFound("Not Found".to_string()).into_response()
}
