// src/serial.rs

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use crate::{
    models::{ArduinoPayload, IrrigationTrigger},
    state::AppState,
};

/// Estrutura do comando enviado pelas rotas para o Daemon
pub struct SerialCommand {
    pub cmd: String,
    pub reply: oneshot::Sender<bool>,
}

pub struct SerialDaemon;

impl SerialDaemon {
    pub async fn run(state: AppState, mut cmd_rx: mpsc::Receiver<SerialCommand>) -> anyhow::Result<()> {
        let port_name = std::env::var("SERIAL_PORT")
            .unwrap_or_else(|_| "/dev/ttyUSB0".to_string());
        let baud_rate: u32 = std::env::var("SERIAL_BAUD")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(9600);

        tracing::info!("Daemon serial iniciado em {port_name} @ {baud_rate}bps");

        let mut estados: HashMap<Uuid, bool> = HashMap::new();

        loop {
            // 1. Limpa comandos parados na fila enquanto estava desconectado
            while let Ok(req) = cmd_rx.try_recv() {
                let _ = req.reply.send(false);
            }

            let port_name_clone = port_name.clone();

            // Abre a porta com um timeout bem curto (10ms) para não travar as outras tarefas
            match serialport::new(&port_name_clone, baud_rate)
                .timeout(Duration::from_millis(10))
                .open() 
            {
                Ok(mut port) => {
                    tracing::info!("Conectado com sucesso ao Arduino em {}", port_name_clone);
                    let mut buf = String::new();
                    let mut byte = [0u8; 1];

                    loop {
                        // 2. Recebe ordens das Rotas e escreve no Arduino
                        while let Ok(req) = cmd_rx.try_recv() {
                            match port.write_all(req.cmd.as_bytes()) {
                                Ok(_) => {
                                    let _ = port.flush();
                                    let _ = req.reply.send(true); // Avisa a rota que deu certo
                                }
                                Err(e) => {
                                    tracing::error!("Falha ao escrever na serial: {}", e);
                                    let _ = req.reply.send(false); // Avisa a rota que falhou
                                }
                            }
                        }

                        // 3. Lê dados do Arduino e repassa pro banco
                        match port.read(&mut byte) {
                            Ok(1) => {
                                let ch = byte[0] as char;
                                if ch == '\n' {
                                    if !buf.trim().is_empty() {
                                        process_line(buf.trim(), &state, &mut estados).await;
                                    }
                                    buf.clear();
                                } else {
                                    buf.push(ch);
                                }
                            }
                            Ok(_) => {} // EOF silencioso
                            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                // Timeout super curto (10ms) esperado, apenas permite que o tokio respire
                                tokio::task::yield_now().await;
                            }
                            Err(e) => {
                                tracing::warn!("Conexão serial perdida: {}", e);
                                break; // Sai do loop interno e entra na espera para reconectar
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Aguardando Arduino na porta {}: {}", port_name_clone, e);
                    tokio::time::sleep(Duration::from_secs(3)).await; // Espera 3s antes de tentar abrir de novo
                }
            }
        }
    }
}

async fn process_line(line: &str, state: &AppState, estados: &mut HashMap<Uuid, bool>) {
    let payload: ArduinoPayload = match serde_json::from_str(line) {
        Ok(p)  => p,
        Err(e) => { tracing::warn!("JSON inválido: '{line}' — {e}"); return; }
    };

    if let Err(e) = payload.validate() {
        tracing::warn!("Leitura descartada: {e}"); return;
    }

    let plant = match state.db().find_plant_by_name(&payload.plant_name).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            tracing::warn!("Planta '{}' não encontrada", payload.plant_name);
            return;
        }
        Err(e) => { tracing::error!("Erro ao buscar planta: {e}"); return; }
    };

    let luz_agora = payload.luz_ligada == 1;
    let luz_antes = estados.get(&plant.id).copied();

    match luz_antes {
        None => {
            if luz_agora {
                if let Err(e) = state.db().luz_abrir_periodo(plant.id).await {
                    tracing::error!("Erro ao abrir período de luz: {e}");
                }
                tracing::info!(plant = %plant.name, "Primeira leitura — luz ligada");
            }
        }
        Some(false) if luz_agora => {
            if let Err(e) = state.db().luz_abrir_periodo(plant.id).await {
                tracing::error!("Erro ao abrir período de luz: {e}");
            }
            tracing::info!(plant = %plant.name, "Luz ligou");
        }
        Some(true) if !luz_agora => {
            if let Err(e) = state.db().luz_fechar_periodo(plant.id).await {
                tracing::error!("Erro ao fechar período de luz: {e}");
            }
            tracing::info!(plant = %plant.name, "Luz desligou");
        }
        _ => {}
    }

    estados.insert(plant.id, luz_agora);

    tracing::debug!(
        plant = %plant.name,
        humidity = payload.humidity,
        luz_ligada = payload.luz_ligada,
        "Nova leitura recebida"
    );

    if let Err(e) = state.db().insert_reading(
        plant.id,
        payload.humidity,
        payload.light_lux,
        payload.luz_ligada,
    ).await {
        tracing::error!("Erro ao salvar leitura: {e}"); return;
    }

    if let Err(e) = state.db().set_luz_status(plant.id, luz_agora).await {
        tracing::error!("Erro ao salvar status de luz: {e}");
    }

    if payload.humidity < plant.humidity_min {
        tracing::info!(plant = %plant.name, "Umidade baixa — irrigação automática");
        if let Err(e) = state.db().insert_irrigation_log(plant.id, IrrigationTrigger::Auto, 10, None).await {
            tracing::error!("Erro ao registrar irrigação: {e}");
        }
    }
}