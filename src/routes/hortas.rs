// src/routes/hortas.rs

use axum::{Json, extract::{Path, State}};
use chrono::Utc;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::{HortaResponse, IrrigationTrigger},
    routes::util::normalize_plant_name,
    state::AppState,
};

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub code:       String,
    pub plant_name: String,
}

#[derive(Deserialize)]
pub struct PatchPlantRequest {
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

    // Normaliza o nome antes de buscar, assim "Manjericao" e "Manjericão"
    // resolvem para a mesma planta e não criam duplicatas.
    let plant_name_normalized = normalize_plant_name(&req.plant_name);

    // Busca ou cria a planta pelo nome normalizado
    let plant = match state.db().find_plant_by_normalized_name(&plant_name_normalized).await? {
        Some(p) => p,
        None => {
            // Salva com o nome original fornecido (mantém acentos na UI)
            state.db().create_plant(
                &req.plant_name,
                None,
                60.0, 80.0,
                10.0,
                user.0.sub,
                false, // planta criada pelo usuário → privada
            ).await?
        }
    };

    let horta = state.db().create_horta(&code, plant.id, user.0.sub).await?;

    // Inicializa umidade no meio do range da planta (só se ainda não existir)
    state.db().init_umidade_status(plant.id, plant.humidity_min, plant.humidity_max).await?;

    // Para plantas não-Manjericão, inicializa histórico de luz com valor aleatório
    // para não começar do zero (mesmo comportamento da umidade fictícia)
    let manjericao = state.db().find_plant_by_normalized_name("manjericao").await?;
    let is_manjericao = manjericao.as_ref().map(|m| m.id) == Some(plant.id);
    if !is_manjericao {
        let _ = state.db().init_luz_historico(plant.id).await;
    }

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

/// PATCH /hortas/:code/plant
/// Altera a planta associada a uma horta existente.
/// O usuário só pode alterar suas próprias hortas.
pub async fn patch_plant(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
    Json(req): Json<PatchPlantRequest>,
) -> AppResult<Json<HortaResponse>> {
    let plant_name = req.plant_name.trim().to_string();
    if plant_name.is_empty() {
        return Err(AppError::BadRequest("Nome da planta é obrigatório".to_string()));
    }

    // Garante que a horta pertence ao usuário autenticado
    let horta = state.db()
        .find_horta_by_code(&code).await?
        .ok_or_else(|| AppError::NotFound(format!("Horta '{code}' não encontrada")))?;

    if horta.owner_id != user.0.sub {
        return Err(AppError::Forbidden);
    }

    // Busca a planta pelo nome (normalizado para evitar duplicatas)
    let plant_name_normalized = normalize_plant_name(&plant_name);
    let plant = state.db()
        .find_plant_by_normalized_name(&plant_name_normalized).await?
        .ok_or_else(|| AppError::NotFound(format!("Planta '{plant_name}' não encontrada")))?;

    // Persiste a troca no banco
    state.db().update_horta_plant(horta.id, plant.id).await?;

    Ok(Json(HortaResponse {
        id:         horta.id,
        code:       horta.code,
        plant_name: plant.name,
        owner_id:   horta.owner_id,
        created_at: horta.created_at.to_rfc3339(),
    }))
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

    // 1. Pega os dados da planta que o usuário escolheu na tela (ex: Alecrim, Hortelã, etc)
    let plant = state.db()
        .get_plant(horta.plant_id).await?
        .ok_or_else(|| AppError::NotFound("Planta não encontrada".to_string()))?;

    // === INÍCIO DA GAMBIARRA ACADÊMICA ===
    let mut target_plant_id = plant.id;

    if let Some(manjericao) = state.db().find_plant_by_normalized_name("manjericao").await? {
        target_plant_id = manjericao.id; // "Rouba" o ID para puxar os dados do sensor
    }

    // Busca os dados do sensor usando o ID do Manjericão (luz, lux, etc.)
    let sensor_reading     = state.db().latest_reading(target_plant_id).await?;
    let recent_logs        = state.db().list_irrigation_logs(plant.id, 5).await?;

    let latest_reading = if plant.id == target_plant_id {
        sensor_reading
    } else {
        let (umidade_planta, ultima_atualizacao) = state.db()
            .get_umidade_status_com_tempo(plant.id).await?
            .unwrap_or_else(|| {
                let mid = (plant.humidity_min + plant.humidity_max) / 2.0;
                (mid, horta.created_at)
            });

        let agora = Utc::now();
        let minutos_passados = agora.signed_duration_since(ultima_atualizacao).num_minutes() as f64;
        let taxa_secagem_por_minuto = 0.05;

        let umidade_calculada = (umidade_planta - (minutos_passados * taxa_secagem_por_minuto)).max(0.0);

        sensor_reading.map(|mut r| { r.humidity = umidade_calculada; r })
    };

    let recent_logs        = state.db().list_irrigation_logs(plant.id, 5).await?;
    let luz_total_hoje_seg = state.db().luz_total_hoje(plant.id).await.unwrap_or(0);
    let luz_ligada         = state.db().get_luz_status(plant.id).await.unwrap_or(false);
    // === FIM DA GAMBIARRA ===

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
        luz_total_hoje_seg,
        luz_ligada,
        fetched_at: Utc::now(),
    }))
}

/// POST /hortas/:code/regar
pub async fn regar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let horta = state.db()
        .find_horta_by_code(&code).await?
        .ok_or_else(|| AppError::NotFound(format!("Horta '{code}' não encontrada")))?;

    let mut target_plant_id = horta.plant_id;
    let is_manjericao = if let Some(manjericao) = state.db().find_plant_by_normalized_name("manjericao").await? {
        if horta.plant_id == manjericao.id {
            target_plant_id = manjericao.id;
            
            if user.0.role != crate::models::UserRole::Admin {
                return Err(AppError::Forbidden);
            }
            true
        } else {
            false
        }
    } else {
        false
    };

    let ultima_leitura = state.db().latest_reading(target_plant_id).await?;
    let light_lux      = ultima_leitura.as_ref().map(|r| r.light_lux).unwrap_or(0.0);
    let luz_ligada     = ultima_leitura.as_ref().map(|r| r.luz_ligada).unwrap_or(0);

    let plant = state.db().get_plant(horta.plant_id).await?
        .ok_or_else(|| AppError::NotFound("Planta não encontrada".to_string()))?;

    let umidade_atual = if is_manjericao {
        ultima_leitura.as_ref().map(|r| r.humidity).unwrap_or((plant.humidity_min + plant.humidity_max) / 2.0)
    } else {
        let (umidade_bd, ultima_atualizacao) = state.db()
            .get_umidade_status_com_tempo(horta.plant_id).await?
            .unwrap_or_else(|| {
                let mid = (plant.humidity_min + plant.humidity_max) / 2.0;
                (mid, horta.created_at)
            });

        let agora = Utc::now();
        let minutos_passados = agora.signed_duration_since(ultima_atualizacao).num_minutes() as f64;
        let taxa_secagem_por_minuto = 0.05;

        (umidade_bd - (minutos_passados * taxa_secagem_por_minuto)).max(0.0)
    };

    if umidade_atual >= 100.0 {
        return Err(AppError::BadRequest(
            "A umidade já está em 100%. Não é necessário regar agora.".to_string(),
        ));
    }

    let incremento = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
        7.0 + (seed % 8_000_000) as f64 / 1_000_000.0
    };
    let nova_umidade = (umidade_atual + incremento).min(100.0);
    let duration_sec = (incremento * 2.0).round() as i32;

    // --- CORREÇÃO DO HARDWARE (RETORNANDO ERRO PARA A TELA VIA MPSC) ---
    if is_manjericao {
        let cmd = format!("IRRIGAR {}\n", duration_sec);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let req = crate::serial::SerialCommand { cmd, reply: reply_tx };
        
        // Tenta enviar o comando para a fila. 
        // Se a fila cair ou o Arduino responder 'false' (offline), barra tudo.
        if state.serial_tx().send(req).await.is_err() || !reply_rx.await.unwrap_or(false) {
            tracing::error!("Falha ao tentar acionar a bomba: Arduino offline");
            return Err(AppError::BadRequest(
                "Arduino offline! Verifique a conexão USB para regar o Manjericão.".to_string()
            ));
        }
    }
    // -------------------------------------------------------------------

    let log = state.db()
        .insert_irrigation_log(horta.plant_id, IrrigationTrigger::Manual, duration_sec, Some(user.0.sub))
        .await?;

    state.db().set_umidade_status(horta.plant_id, nova_umidade).await?;

    state.db()
        .insert_reading(target_plant_id, nova_umidade, light_lux, luz_ligada)
        .await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "umidade_anterior": umidade_atual,
        "umidade_nova": nova_umidade,
        "incremento": incremento,
        "duration_sec": duration_sec,
        "log_id": log.id.to_string(),
    })))
}