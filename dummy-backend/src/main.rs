use axum::body::Body;
use axum::http::Request;
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: u16,
}

async fn process(request: Request<Body>) -> impl IntoResponse {
    let request_id = request
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();

    println!("Request received {}", request_id)
}

async fn health() -> impl IntoResponse {
    "PONG"
}

fn router(port: u16) -> Router {
    let state = Args { port };
    Router::new()
        .route("/", get(process))
        .route("/health", get(health))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();
    axum::serve(listener, router(args.port)).await.unwrap();
}
