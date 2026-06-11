// src/routes/admin.rs
//
// Controle de luz — acessível a qualquer usuário autenticado
// que seja dono da horta, ou a admins para qualquer horta.

use axum::{Json, extract::{Path, State}};
use serde_json::{Value, json};
use std::collections::HashMap;

use chrono::Utc;

use crate::{
    auth::AuthUser,
    errors::{AppError, AppResult},
    models::UserRole,
    state::AppState,
};

/// POST /admin/luz/:code/ligar
pub async fn luz_ligar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<Value>> {
    let horta = verificar_acesso(&state, &code, &user).await?;

    // Usuários comuns têm cooldown de 30s no Manjericão (planta física)
    // para evitar spam de comandos e danos ao hardware
    if user.0.role != UserRole::Admin {
        if let Some(manjericao) = state.db().find_plant_by_normalized_name("manjericao").await? {
            if horta.plant_id == manjericao.id {
                verificar_cooldown_luz(&state, horta.id).await?;
            }
        }
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

    // Tenta enviar pelo Arduino; se offline, apenas salva no banco
    match enviar_serial("LUZ_ON") {
        Ok(()) => tracing::info!(horta = code, "LUZ_ON enviado via serial"),
        Err(_)  => tracing::warn!(horta = code, "Arduino offline — estado salvo só no banco"),
    }
    let _ = state.db().set_luz_ligada(horta.plant_id, true).await;
    if let Err(e) = state.db().luz_abrir_periodo(horta.plant_id).await {
        tracing::error!("Erro ao abrir período de luz: {e}");
    }
    if user.0.role != UserRole::Admin {
        let _ = state.db().set_luz_cooldown(horta.id).await;
    }
    Ok(Json(json!({ "ok": true, "acao": "ligar", "horta": code })))
}

/// POST /admin/luz/:code/desligar
pub async fn luz_desligar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<Value>> {
    let horta = verificar_acesso(&state, &code, &user).await?;

    match enviar_serial("LUZ_OFF") {
        Ok(()) => tracing::info!(horta = code, "LUZ_OFF enviado via serial"),
        Err(_)  => tracing::warn!(horta = code, "Arduino offline — estado salvo só no banco"),
    }
    let _ = state.db().set_luz_ligada(horta.plant_id, false).await;
    if let Err(e) = state.db().luz_fechar_periodo(horta.plant_id).await {
        tracing::error!("Erro ao fechar período de luz: {e}");
    }
    Ok(Json(json!({ "ok": true, "acao": "desligar", "horta": code })))
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

fn enviar_serial(cmd: &str) -> AppResult<()> {
    let port_name = std::env::var("SERIAL_PORT")
        .unwrap_or_else(|_| "/dev/ttyUSB0".to_string());
    let baud_rate: u32 = std::env::var("SERIAL_BAUD")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(9600);

    let cmd_str = format!("{}\n", cmd);
    let result = std::panic::catch_unwind(|| {
        let mut port = serialport::new(&port_name, baud_rate)
            .timeout(std::time::Duration::from_millis(2000))
            .open()?;
        use std::io::Write;
        port.write_all(cmd_str.as_bytes())?;
        port.flush()?;
        Ok::<(), anyhow::Error>(())
    });

    match result {
        Ok(Ok(())) => { tracing::info!(cmd = cmd, "Comando serial enviado"); Ok(()) }
        Ok(Err(e)) => Err(AppError::Internal(anyhow::anyhow!("Serial: {e}"))),
        Err(_)     => Err(AppError::Internal(anyhow::anyhow!("Porta serial indisponível"))),
    }
}