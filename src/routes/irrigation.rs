use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::{IrrigationLog, IrrigationTrigger, ManualIrrigationRequest, UserRole},
    state::AppState,
};

/// POST /irrigation/manual
pub async fn trigger_manual(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<ManualIrrigationRequest>,
) -> AppResult<Json<IrrigationLog>> {
    if req.duration_sec < 1 || req.duration_sec > 300 {
        return Err(AppError::BadRequest(
            "Duração deve estar entre 1 e 300 segundos".to_string(),
        ));
    }

    state.db()
        .get_plant(req.plant_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Planta {} não encontrada", req.plant_id)))?;

    let is_admin = user.0.role == UserRole::Admin;

    let is_manjericao = state.db()
        .find_plant_by_normalized_name("manjericao").await?
        .map(|p| p.id == req.plant_id)
        .unwrap_or(false);

    if !is_admin && is_manjericao {
        return Err(AppError::Forbidden);
    }

    if is_manjericao {
        let cmd = format!("IRRIGAR {}\n", req.duration_sec);
        
        // Cria um canal de resposta rápida (oneshot)
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let command = crate::serial::SerialCommand { cmd, reply: reply_tx };

        // Envia para o Daemon da serial e aguarda a resposta
        match state.serial_tx().send(command).await {
            Ok(_) => {
                if reply_rx.await.unwrap_or(false) {
                    tracing::info!(duration = req.duration_sec, "Irrigação manual enviada via serial");
                } else {
                    tracing::warn!("Arduino offline — irrigação registrada só no banco");
                }
            }
            Err(_) => tracing::warn!("Arduino offline — irrigação registrada só no banco"),
        }
    }

    tracing::info!(
        plant_id = %req.plant_id,
        duration = req.duration_sec,
        admin = is_admin,
        "Irrigação manual solicitada"
    );

    let log = state.db()
        .insert_irrigation_log(req.plant_id, IrrigationTrigger::Manual, req.duration_sec, Some(user.0.sub))
        .await?;

    Ok(Json(log))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 { 20 }

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