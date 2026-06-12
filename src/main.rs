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
    http::Method,
};
use dotenvy::dotenv;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{db::Database, routes::build_router, serial::{SerialDaemon, SerialCommand}, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mini_horta=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database = Database::connect().await?;
    info!("Banco de dados conectado e migrations aplicadas");

    let (serial_tx, serial_rx) = tokio::sync::mpsc::channel::<SerialCommand>(32);
    let state = AppState::new(database, serial_tx);

    // ── Daemon serial (Arduino) ────────────────────────────────────────────────
    let serial_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = SerialDaemon::run(serial_state, serial_rx).await {
            tracing::error!("Daemon serial encerrou com erro: {e}");
        }
    });

    // ── Task de reset de luz à meia-noite ──────────────────────────────────────
    let reset_state = state.clone();
    tokio::spawn(async move {
        loop {
            let agora = chrono::Utc::now();

            // Calcula quanto tempo falta para a próxima meia-noite UTC
            let proxima_meia_noite = (agora + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();

            let espera = (proxima_meia_noite - agora)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60));

            tracing::info!(
                "Reset de luz agendado para {} (em {:.0} min)",
                proxima_meia_noite,
                espera.as_secs_f64() / 60.0
            );

            tokio::time::sleep(espera).await;

            // Fecha todos os períodos abertos
            if let Err(e) = reset_state.db().luz_fechar_todos_periodos().await {
                tracing::error!("Erro no reset de luz à meia-noite: {e}");
            } else {
                tracing::info!("Reset de luz à meia-noite concluído");
            }

            // Aguarda 2s para não disparar duas vezes no mesmo segundo
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });

    // ── CORS ───────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // ── Router ─────────────────────────────────────────────────────────────────
    let app: Router = build_router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
        .parse()?;

    info!("Servidor ouvindo em http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}