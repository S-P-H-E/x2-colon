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
use x2_colon_api::parser::{ParseOutput, calculate_durations, clean_script};

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

#[derive(Deserialize, Validate)]
struct CleanRequest {
    #[validate(length(min = 1))]
    script: String,
}

#[derive(Serialize)]
struct CleanResponse {
    cleaned: String,
}

async fn timestamp(Json(payload): Json<TimeRequest>) -> Result<Json<ParseOutput>, (StatusCode, String)> {
    payload
        .validate()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let result = calculate_durations(&payload.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    
    if result.lines.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No valid timestamps found".to_string()));
    }

    Ok(Json(result))
}

async fn clean(Json(payload): Json<CleanRequest>) -> Result<Json<CleanResponse>, (StatusCode, String)> {
    payload
        .validate()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let cleaned = clean_script(&payload.script);
    Ok(Json(CleanResponse { cleaned }))
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
        .route("/clean", post(clean))
        .route("/favicon.ico", get(favicon))
        .layer(cors);

    let app = ServiceBuilder::new()
        .layer(VercelLayer::new())
        .service(router);
    vercel_runtime::run(app).await
}
