// src/routes/hortas.rs

use axum::{Json, extract::{Path, State}};
use chrono::Utc;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::HortaResponse,
    state::AppState,
};

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub code:       String,
    pub plant_name: String,
}

/// POST /hortas/connect
/// Conecta uma horta pelo código e nome da planta.
/// Se a planta não existir ainda, cria com valores padrão.
pub async fn connect(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<ConnectRequest>,
) -> AppResult<Json<HortaResponse>> {
    let code = req.code.trim().to_string();
    if code.is_empty() {
        return Err(AppError::BadRequest("Código inválido".to_string()));
    }

    if state.db().find_horta_by_code(&code).await?.is_some() {
        return Err(AppError::BadRequest("Código já cadastrado".to_string()));
    }

    // Busca ou cria a planta pelo nome
    let plant = match state.db().find_plant_by_name(&req.plant_name).await? {
        Some(p) => p,
        None => {
            state.db().create_plant(
                &req.plant_name,
                None,
                60.0, 80.0,
                user.0.sub,
            ).await?
        }
    };

    let horta = state.db().create_horta(&code, plant.id, user.0.sub).await?;

    Ok(Json(HortaResponse {
        id:         horta.id,
        code:       horta.code,
        plant_name: plant.name,
        owner_id:   horta.owner_id,
        created_at: horta.created_at.to_rfc3339(),
    }))
}

/// GET /hortas/mine
pub async fn mine(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<HortaResponse>>> {
    let hortas = state.db().list_hortas_by_owner(user.0.sub).await?;
    Ok(Json(hortas))
}

/// GET /hortas/:code/dashboard
pub async fn dashboard(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<super::dashboard::DashboardResponse>> {
    let horta = state.db()
        .find_horta_by_code(&code).await?
        .ok_or_else(|| AppError::NotFound(format!("Horta '{code}' não encontrada")))?;

    let plant = state.db()
        .get_plant(horta.plant_id).await?
        .ok_or_else(|| AppError::NotFound("Planta não encontrada".to_string()))?;

    let latest_reading = state.db().latest_reading(plant.id).await?;
    let recent_logs    = state.db().list_irrigation_logs(plant.id, 5).await?;

    let (status, health_pct) = match &latest_reading {
        None    => (super::dashboard::PlantStatus::SemLeitura, 0),
        Some(r) => super::dashboard::calculate_status(r.humidity, plant.humidity_min, plant.humidity_max),
    };

    tracing::debug!("latest_reading: {:?}", latest_reading);
    tracing::debug!("recent_logs: {:?}", recent_logs);

    Ok(Json(super::dashboard::DashboardResponse {
        plant,
        latest_reading,
        recent_logs,
        status,
        health_pct,
        fetched_at: Utc::now(),
    }))
}
