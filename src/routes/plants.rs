use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    errors::{AppError, AppResult},
    models::{CreatePlantRequest, UpdatePlantRequest, Plant},
    routes::util::normalize_plant_name,
    state::AppState,
};

/// GET /plants — admin vê todas; usuário comum vê públicas + suas próprias
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<Plant>>> {
    let plants = if user.0.role == crate::models::UserRole::Admin {
        state.db().list_all_plants().await?
    } else {
        state.db().list_plants(user.0.sub).await?
    };
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

    // Limite de tamanho para o campo de benefícios/descrição
    if let Some(desc) = &req.description {
        if desc.chars().count() > 300 {
            return Err(AppError::BadRequest(
                "Benefícios deve ter no máximo 300 caracteres".to_string(),
            ));
        }
    }

    // Impede criar uma planta com nome já existente (normalizado, ignora acentos/caixa)
    let normalized = normalize_plant_name(&req.name);
    if state.db().find_plant_by_normalized_name(&normalized).await?.is_some() {
        return Err(AppError::BadRequest(format!("Planta '{}' já existe", req.name)));
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

    if req.luz_horas_dia <= 0.0 || req.luz_horas_dia > 24.0 {
        return Err(AppError::BadRequest("Horas de luz deve estar entre 0 e 24".to_string()));
    }

    let plant = state
        .db()
        .create_plant(
            &req.name,
            req.description.as_deref(),
            req.humidity_min,
            req.humidity_max,
            req.luz_horas_dia,
            admin.0.sub,
            true,
        )
        .await?;

    Ok(Json(plant))
}

/// PUT /plants/:id — somente admin pode editar plantas existentes
pub async fn update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlantRequest>,
) -> AppResult<Json<Plant>> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("Nome da planta é obrigatório".to_string()));
    }

    // Limite de tamanho para o campo de benefícios/descrição
    if let Some(desc) = &req.description {
        if desc.chars().count() > 300 {
            return Err(AppError::BadRequest(
                "Benefícios deve ter no máximo 300 caracteres".to_string(),
            ));
        }
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

    state.db().get_plant(id).await?
        .ok_or_else(|| AppError::NotFound(format!("Planta {id} não encontrada")))?;

    if req.luz_horas_dia <= 0.0 || req.luz_horas_dia > 24.0 {
        return Err(AppError::BadRequest("Horas de luz deve estar entre 0 e 24".to_string()));
    }

    let plant = state
        .db()
        .update_plant(id, &req.name, req.description.as_deref(), req.humidity_min, req.humidity_max, req.luz_horas_dia)
        .await?;

    Ok(Json(plant))
}
/// DELETE /plants/:id — somente admin pode excluir plantas
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    state.db().get_plant(id).await?
        .ok_or_else(|| AppError::NotFound(format!("Planta {id} não encontrada")))?;

    state.db().delete_plant(id).await?;

    Ok(StatusCode::NO_CONTENT)
}
