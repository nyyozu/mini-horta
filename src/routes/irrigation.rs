use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::{IrrigationLog, IrrigationTrigger, ManualIrrigationRequest},
    state::AppState,
};

/// POST /irrigation/manual
/// Aciona a bomba manualmente (qualquer usuário autenticado).
pub async fn trigger_manual(
    State(state): State<AppState>,
    _user: AuthUser,
    Json(req): Json<ManualIrrigationRequest>,
) -> AppResult<Json<IrrigationLog>> {
    if req.duration_sec < 1 || req.duration_sec > 300 {
        return Err(AppError::BadRequest(
            "Duração deve estar entre 1 e 300 segundos".to_string(),
        ));
    }

    // Verifica que a planta existe
    state
        .db()
        .get_plant(req.plant_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Planta {} não encontrada", req.plant_id)))?;

    // Aqui você enviaria o comando para o Arduino via serial.
    // O SerialDaemon expõe um canal (mpsc::Sender) no AppState para isso.
    // Exemplo: state.serial_tx().send(SerialCommand::Irrigate { plant_id, duration_sec }).await?
    // Por ora, apenas registramos o log.
    tracing::info!(
        plant_id = %req.plant_id,
        duration = req.duration_sec,
        "Irrigação manual solicitada"
    );

    let log = state
        .db()
        .insert_irrigation_log(req.plant_id, IrrigationTrigger::Manual, req.duration_sec)
        .await?;

    Ok(Json(log))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    20
}

/// GET /irrigation/:plant_id/logs?limit=20
pub async fn logs(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(plant_id): Path<Uuid>,
    Query(params): Query<LogsQuery>,
) -> AppResult<Json<Vec<IrrigationLog>>> {
    let limit = params.limit.clamp(1, 200);
    let logs = state.db().list_irrigation_logs(plant_id, limit).await?;
    Ok(Json(logs))
}
