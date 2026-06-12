pub mod admin;
pub mod auth;
pub mod dashboard;
pub mod hortas;
pub mod plants;
pub mod sensors;
pub mod irrigation;
pub mod ws;

pub mod util;

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
        .nest("/admin",     admin_routes())
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
        .route("/:id", get(plants::get_one).put(plants::update).delete(plants::delete))
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

fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/hortas",              get(admin::listar_hortas_cadastradas))
        .route("/plants",              get(admin::listar_plantas_admin))
        .route("/luz/:code/ligar",     post(admin::luz_ligar))
        .route("/luz/:code/desligar",  post(admin::luz_desligar))
        .route("/luz/:code/historico", get(admin::luz_historico))
        .route("/luz/:plant_id/logs",  get(admin::luz_logs))
}

fn hortas_routes() -> Router<AppState> {
    Router::new()
        .route("/connect",         post(hortas::connect))
        .route("/mine",            get(hortas::mine))
        .route("/:code/plant",     axum::routing::patch(hortas::patch_plant))
        .route("/:code/dashboard", get(hortas::dashboard))
        .route("/:code/regar",     post(hortas::regar))
}