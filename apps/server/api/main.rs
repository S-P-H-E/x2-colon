use axum::{
    Json, Router,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use validator::Validate;
use vercel_runtime::Error;
use vercel_runtime::axum::VercelLayer;

async fn favicon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../public/favicon.ico").as_slice(),
    )
}

async fn hello() -> impl IntoResponse {
    Json(json!({ "message": "Welcome to x2-colon!" }))
}

#[derive(Deserialize, Validate)]
struct TimeRequest {
    #[validate(length(min = 2))]
    content: String,
}

#[derive(Serialize)]
struct TimeResponse {
    duration: i32,
}

async fn timestamp(
    Json(payload): Json<TimeRequest>,
) -> Result<Json<TimeResponse>, (StatusCode, String)> {
    payload
        .validate()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let _content = &payload.content;
    let duration = 10;

    Ok(Json(TimeResponse { duration }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    // Add CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/", get(hello))
        .route("/timestamp", post(timestamp))
        .route("/favicon.ico", get(favicon))
        .layer(cors);

    let app = ServiceBuilder::new()
        .layer(VercelLayer::new())
        .service(router);
    vercel_runtime::run(app).await
}
