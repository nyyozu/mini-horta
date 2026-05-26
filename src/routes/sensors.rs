use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::SensorReading,
    state::AppState,
};

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    50
}

/// GET /sensors/:plant_id/latest
pub async fn latest(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(plant_id): Path<Uuid>,
) -> AppResult<Json<SensorReading>> {
    let reading = state
        .db()
        .latest_reading(plant_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Nenhuma leitura encontrada".to_string()))?;

    Ok(Json(reading))
}

/// GET /sensors/:plant_id/history?limit=50
pub async fn history(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(plant_id): Path<Uuid>,
    Query(params): Query<HistoryQuery>,
) -> AppResult<Json<Vec<SensorReading>>> {
    let limit = params.limit.clamp(1, 500);
    let readings = state.db().list_readings(plant_id, limit).await?;
    Ok(Json(readings))
}
