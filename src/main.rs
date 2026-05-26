mod auth;
mod db;
mod errors;
mod models;
mod routes;
mod serial;
mod state;

use std::net::SocketAddr;

use axum::{
    Router,
    http::{HeaderValue, Method},
};
use dotenvy::dotenv;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{db::Database, routes::build_router, serial::SerialDaemon, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Carrega .env (DATABASE_URL, JWT_SECRET, SERIAL_PORT, etc.)
    dotenv().ok();

    // Inicializa logs com RUST_LOG (ex.: "mini_horta=debug,tower_http=info")
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mini_horta=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Conecta ao banco e executa migrations automaticamente
    let database = Database::connect().await?;
    info!("Banco de dados conectado e migrations aplicadas");

    // Estado compartilhado entre handlers (DB pool + config JWT)
    let state = AppState::new(database);

    // Inicia daemon de leitura serial em background (Arduino)
    let serial_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = SerialDaemon::run(serial_state).await {
            tracing::error!("Daemon serial encerrou com erro: {e}");
        }
    });

    // Configura CORS (ajuste as origens em produção)
    let cors = CorsLayer::new()
        .allow_origin(
            std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string())
                .parse::<HeaderValue>()?,
        )
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // Monta o router completo
    let app: Router = build_router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Bind e serve
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
        .parse()?;

    info!("Servidor ouvindo em http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
