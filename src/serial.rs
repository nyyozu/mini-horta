use std::time::Duration;

use tokio::task;

use crate::{
    models::{ArduinoPayload, IrrigationTrigger},
    state::AppState,
};

/// Daemon que roda em background lendo a porta serial do Arduino.
/// A leitura bloqueante do serialport é executada em `spawn_blocking`
/// para não bloquear o runtime Tokio.
pub struct SerialDaemon;

impl SerialDaemon {
    pub async fn run(state: AppState) -> anyhow::Result<()> {
        let port_name = std::env::var("SERIAL_PORT")
            .unwrap_or_else(|_| "/dev/ttyUSB0".to_string());
        let baud_rate: u32 = std::env::var("SERIAL_BAUD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(9600);

        tracing::info!("Daemon serial iniciado em {port_name} @ {baud_rate}bps");

        loop {
            let port_name_clone = port_name.clone();
            let state_clone = state.clone();

            let result = task::spawn_blocking(move || -> anyhow::Result<String> {
                let mut port = serialport::new(&port_name_clone, baud_rate)
                    .timeout(Duration::from_millis(2000))
                    .open()?;

                let mut buf = String::new();
                let mut byte = [0u8; 1];

                // Lê byte a byte até encontrar '\n' (Arduino envia JSON + \n)
                loop {
                    match port.read(&mut byte) {
                        Ok(1) => {
                            let ch = byte[0] as char;
                            if ch == '\n' {
                                break;
                            }
                            buf.push(ch);
                        }
                        Ok(_) | Err(_) => break,
                    }
                }

                Ok(buf)
            })
            .await;

            match result {
                Ok(Ok(line)) if !line.trim().is_empty() => {
                    process_line(line.trim(), &state).await;
                }
                Ok(Err(e)) => {
                    tracing::warn!("Porta serial indisponível: {e}. Tentando novamente em 5s...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tracing::error!("spawn_blocking falhou: {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                _ => {}
            }
        }
    }
}

/// Processa uma linha JSON recebida do Arduino.
async fn process_line(line: &str, state: &AppState) {
    let payload: ArduinoPayload = match serde_json::from_str(line) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("JSON inválido do Arduino: '{line}' — {e}");
            return;
        }
    };

    // Validação de faixa
    if let Err(e) = payload.validate() {
        tracing::warn!("Leitura descartada: {e}");
        return;
    }

    tracing::debug!(
        plant = %payload.plant_id,
        humidity = payload.humidity,
        lux = payload.light_lux,
        "Nova leitura recebida"
    );

    // Persiste a leitura
    if let Err(e) = state
        .db()
        .insert_reading(payload.plant_id, payload.humidity, payload.light_lux)
        .await
    {
        tracing::error!("Erro ao salvar leitura: {e}");
        return;
    }

    // Verifica threshold e dispara irrigação automática se necessário
    check_and_irrigate(&payload, state).await;
}

async fn check_and_irrigate(payload: &ArduinoPayload, state: &AppState) {
    let plant = match state.db().get_plant(payload.plant_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            tracing::warn!("Planta {} não encontrada no banco", payload.plant_id);
            return;
        }
        Err(e) => {
            tracing::error!("Erro ao buscar planta: {e}");
            return;
        }
    };

    if payload.humidity < plant.humidity_min {
        tracing::info!(
            plant = %plant.name,
            humidity = payload.humidity,
            threshold = plant.humidity_min,
            "Umidade abaixo do mínimo — acionando irrigação automática"
        );

        // Duração padrão de 10 segundos; parametrize no Plant se quiser
        let duration_sec = 10i32;

        if let Err(e) = state
            .db()
            .insert_irrigation_log(payload.plant_id, IrrigationTrigger::Auto, duration_sec)
            .await
        {
            tracing::error!("Erro ao registrar log de irrigação: {e}");
        }

        // TODO: enviar comando para Arduino ligar a bomba via serial
        // serial_write_command(&format!("IRRIGATE {} {}\n", payload.plant_id, duration_sec));
        tracing::info!("Comando de irrigação enviado para o Arduino");
    }
}
