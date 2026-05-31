pub mod auth;
pub mod dashboard;
pub mod hortas;
pub mod plants;
pub mod sensors;
pub mod irrigation;
pub mod ws;

use axum::{Router, routing::{get, post}};
use tower_http::services::ServeDir;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/auth",       auth_routes())
        .nest("/plants",     plants_routes())
        .nest("/sensors",    sensors_routes())
        .nest("/irrigation", irrigation_routes())
        .nest("/hortas",     hortas_routes())
        .route("/ws",        get(ws::ws_handler))
        .route("/health",    get(|| async { "ok" }))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login",    post(auth::login))
}

fn plants_routes() -> Router<AppState> {
    Router::new()
        .route("/",    get(plants::list).post(plants::create))
        .route("/:id", get(plants::get_one))
}

fn sensors_routes() -> Router<AppState> {
    Router::new()
        .route("/:plant_id/latest",  get(sensors::latest))
        .route("/:plant_id/history", get(sensors::history))
}

fn irrigation_routes() -> Router<AppState> {
    Router::new()
        .route("/manual",         post(irrigation::trigger_manual))
        .route("/:plant_id/logs", get(irrigation::logs))
}

fn hortas_routes() -> Router<AppState> {
    Router::new()
        .route("/connect",         post(hortas::connect))
        .route("/mine",            get(hortas::mine))
        .route("/:code/dashboard", get(hortas::dashboard))
}