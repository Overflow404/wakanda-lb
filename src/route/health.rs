use axum::response::IntoResponse;
use tracing::info;

pub(crate) async fn health() -> impl IntoResponse {
    info!("Health check executed");
    "PONG"
}
