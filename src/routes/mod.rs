pub mod auth;
pub mod plants;
pub mod sensors;
pub mod irrigation;
pub mod ws;

use axum::{Router, routing::{get, post}};
use tower_http::services::ServeDir;
use crate::state::AppState;

/// Monta o router completo da aplicação.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Autenticação (pública)
        .nest("/auth", auth_routes())
        // Recursos protegidos
        .nest("/plants", plants_routes())
        .nest("/sensors", sensors_routes())
        .nest("/irrigation", irrigation_routes())
        // WebSocket — streaming de leituras em tempo real
        .route("/ws", get(ws::ws_handler))
        // Health check
        .route("/health", get(|| async { "ok" }))
        // Frontend estático — serve tudo em ./static/
        // index.html é servido automaticamente em "/"
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
}

fn plants_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(plants::list).post(plants::create))
        .route("/:id", get(plants::get_one))
}

fn sensors_routes() -> Router<AppState> {
    Router::new()
        .route("/:plant_id/latest", get(sensors::latest))
        .route("/:plant_id/history", get(sensors::history))
}

fn irrigation_routes() -> Router<AppState> {
    Router::new()
        .route("/manual", post(irrigation::trigger_manual))
        .route("/:plant_id/logs", get(irrigation::logs))
}