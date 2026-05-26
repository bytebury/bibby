use askama::Template;
use askama_web::WebTemplate;
use axum::Router;
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::routing::get;
use std::env;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(Template, WebTemplate)]
#[template(path = "homepage.html")]
struct Homepage {
    app_name: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let app_name = env::var("APP_NAME").unwrap_or_else(|_| "Bibby".to_string());

    let serve_static = Router::new()
        .nest_service("/assets", ServeDir::new("public"))
        .layer(SetResponseHeaderLayer::if_not_present(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000"),
        ));

    let app = Router::new()
        .route(
            "/",
            get({
                let app_name = app_name.clone();
                move || async move { Homepage { app_name } }
            }),
        )
        .merge(serve_static)
        .layer(CompressionLayer::new());

    let address = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();
    tracing::info!("listening on http://localhost:{}", port);

    axum::serve(listener, app).await.expect("Failed to start.");
}
