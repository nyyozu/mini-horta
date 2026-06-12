// src/routes/admin.rs
//
// Controle de luz — acessível a qualquer usuário autenticado
// que seja dono da horta, ou a admins para qualquer horta.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::{LuzLog, UserRole},
    serial::SerialCommand,
    state::AppState,
};

/// POST /admin/luz/:code/ligar
pub async fn luz_ligar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<Value>> {
    let horta = verificar_acesso(&state, &code, &user).await?;

    let manjericao = state.db().find_plant_by_normalized_name("manjericao").await?;
    let is_manjericao = manjericao.as_ref().map(|m| m.id) == Some(horta.plant_id);

    // Usuários comuns têm cooldown de 30s no Manjericão (planta física)
    // para evitar spam de comandos e danos ao hardware
    if user.0.role != UserRole::Admin && is_manjericao {
        verificar_cooldown_luz(&state, horta.id).await?;
    }

    // Verificar limite diário de luz da planta
    let plant = state.db()
        .get_plant(horta.plant_id).await?
        .ok_or_else(|| AppError::NotFound("Planta não encontrada".to_string()))?;

    let total_hoje = state.db().luz_total_hoje(horta.plant_id).await.unwrap_or(0);
    let limite_seg = (plant.luz_horas_dia * 3600.0) as i64;

    if total_hoje >= limite_seg {
        return Err(AppError::BadRequest(format!(
            "Limite diário de {}h de luz atingido para {}. Tente novamente amanhã.",
            plant.luz_horas_dia, plant.name
        )));
    }

    let mut token: Option<f64> = None;

    if is_manjericao {
        // Envia pelo canal do daemon serial (não abre a porta diretamente)
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let _ = state.serial_tx().send(SerialCommand { cmd: "LUZ_ON\n".to_string(), reply: reply_tx }).await;
        match reply_rx.await.unwrap_or(false) {
            true  => tracing::info!(horta = code, "LUZ_ON enviado via serial"),
            false => tracing::warn!(horta = code, "Arduino offline — estado salvo só no banco"),
        }
    } else {
        // Plantas que não são o Manjericão não têm hardware próprio:
        // apenas atualizamos o contador/temporizador delas no banco,
        // gerando um token aleatório (mesmo padrão usado na rega).
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
        token = Some(7.0 + (seed % 8_000_000) as f64 / 1_000_000.0);
        tracing::info!(horta = code, token = ?token, "Luz simulada acionada (sem hardware)");
    }

    let _ = state.db().set_luz_ligada(horta.plant_id, true).await;
    if let Err(e) = state.db().luz_abrir_periodo(horta.plant_id).await {
        tracing::error!("Erro ao abrir período de luz: {e}");
    }
    if user.0.role != UserRole::Admin && is_manjericao {
        let _ = state.db().set_luz_cooldown(horta.id).await;
    }

    if let Err(e) = state.db().insert_luz_log(horta.plant_id, "ligar", token, Some(user.0.sub)).await {
        tracing::error!("Erro ao registrar log de luz: {e}");
    }

    Ok(Json(json!({ "ok": true, "acao": "ligar", "horta": code, "token": token })))
}

/// POST /admin/luz/:code/desligar
pub async fn luz_desligar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<Value>> {
    let horta = verificar_acesso(&state, &code, &user).await?;

    let manjericao = state.db().find_plant_by_normalized_name("manjericao").await?;
    let is_manjericao = manjericao.as_ref().map(|m| m.id) == Some(horta.plant_id);

    let mut token: Option<f64> = None;

    if is_manjericao {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let _ = state.serial_tx().send(SerialCommand { cmd: "LUZ_OFF\n".to_string(), reply: reply_tx }).await;
        match reply_rx.await.unwrap_or(false) {
            true  => tracing::info!(horta = code, "LUZ_OFF enviado via serial"),
            false => tracing::warn!(horta = code, "Arduino offline — estado salvo só no banco"),
        }
    } else {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
        token = Some(7.0 + (seed % 8_000_000) as f64 / 1_000_000.0);
        tracing::info!(horta = code, token = ?token, "Luz simulada desligada (sem hardware)");
    }

    let _ = state.db().set_luz_ligada(horta.plant_id, false).await;
    if let Err(e) = state.db().luz_fechar_periodo(horta.plant_id).await {
        tracing::error!("Erro ao fechar período de luz: {e}");
    }

    if let Err(e) = state.db().insert_luz_log(horta.plant_id, "desligar", token, Some(user.0.sub)).await {
        tracing::error!("Erro ao registrar log de luz: {e}");
    }

    Ok(Json(json!({ "ok": true, "acao": "desligar", "horta": code, "token": token })))
}

/// GET /admin/luz/:code/historico?dias=7
pub async fn luz_historico(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    let horta = verificar_acesso(&state, &code, &user).await?;

    let dias: i64 = params.get("dias")
        .and_then(|v| v.parse().ok())
        .unwrap_or(7)
        .clamp(1, 30);

    let hoje      = state.db().luz_total_hoje(horta.plant_id).await?;
    let historico = state.db().luz_historico_dias(horta.plant_id, dias).await?;

    Ok(Json(json!({
        "plant_id":  horta.plant_id,
        "hoje_sec":  hoje,
        "historico": historico,
    })))
}

#[derive(Deserialize)]
pub struct LuzLogsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 { 20 }

/// GET /admin/luz/:plant_id/logs?limit=20
pub async fn luz_logs(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(plant_id): Path<Uuid>,
    Query(params): Query<LuzLogsQuery>,
) -> AppResult<Json<Vec<LuzLog>>> {
    let limit = params.limit.clamp(1, 200);
    let logs = state.db().list_luz_logs(plant_id, limit).await?;
    Ok(Json(logs))
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Verifica se o cooldown de 30s entre comandos de luz foi respeitado.
/// Retorna 429 se o último comando foi há menos de 30 segundos.
async fn verificar_cooldown_luz(state: &AppState, horta_id: uuid::Uuid) -> AppResult<()> {
    const COOLDOWN_SEG: i64 = 30;

    if let Ok(Some(ultimo)) = state.db().get_luz_cooldown(horta_id).await {
        let decorrido = Utc::now().timestamp() - ultimo;
        if decorrido < COOLDOWN_SEG {
            let restante = COOLDOWN_SEG - decorrido;
            return Err(AppError::BadRequest(format!(
                "Aguarde {restante}s antes de enviar outro comando de luz."
            )));
        }
    }
    Ok(())
}

/// Verifica se o usuário tem acesso à horta:
/// - Admins têm acesso a qualquer horta
/// - Usuários comuns só têm acesso às próprias hortas
async fn verificar_acesso(
    state: &AppState,
    code: &str,
    user: &AuthUser,
) -> AppResult<crate::models::Horta> {
    let horta = state.db()
        .find_horta_by_code(code).await?
        .ok_or_else(|| AppError::NotFound(format!("Horta '{code}' não encontrada")))?;

    if user.0.role != UserRole::Admin && horta.owner_id != user.0.sub {
        return Err(AppError::Forbidden);
    }

    Ok(horta)
}
