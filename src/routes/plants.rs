use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    errors::{AppError, AppResult},
    models::{CreatePlantRequest, Plant},
    state::AppState,
};

/// GET /plants — lista todas as plantas (qualquer usuário autenticado)
pub async fn list(
    State(state): State<AppState>,
    _user: AuthUser,
) -> AppResult<Json<Vec<Plant>>> {
    let plants = state.db().list_plants().await?;
    Ok(Json(plants))
}

/// GET /plants/:id
pub async fn get_one(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Plant>> {
    let plant = state
        .db()
        .get_plant(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Planta {id} não encontrada")))?;

    Ok(Json(plant))
}

/// POST /plants — somente admin pode cadastrar novas plantas
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(req): Json<CreatePlantRequest>,
) -> AppResult<Json<Plant>> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("Nome da planta é obrigatório".to_string()));
    }
    if req.humidity_min >= req.humidity_max {
        return Err(AppError::BadRequest(
            "humidity_min deve ser menor que humidity_max".to_string(),
        ));
    }
    if !(0.0..=100.0).contains(&req.humidity_min)
        || !(0.0..=100.0).contains(&req.humidity_max)
    {
        return Err(AppError::BadRequest("Umidade deve estar entre 0 e 100".to_string()));
    }

    let plant = state
        .db()
        .create_plant(
            &req.name,
            req.description.as_deref(),
            req.humidity_min,
            req.humidity_max,
            admin.0.sub,
        )
        .await?;

    Ok(Json(plant))
}
