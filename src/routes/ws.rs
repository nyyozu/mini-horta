use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use serde_json::json;
use tokio::time::{Duration, interval};

use crate::state::AppState;

/// GET /ws
/// Upgrade para WebSocket. Envia a última leitura de todos os sensores
/// a cada 5 segundos para o front-end manter o dashboard em tempo real.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut ticker = interval(Duration::from_secs(5));

    loop {
        ticker.tick().await;

        // Busca todas as plantas e a última leitura de cada uma
        let plants = match state.db().list_all_plants().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("WS: erro ao listar plantas: {e}");
                break;
            }
        };

        let mut readings = Vec::new();
        for plant in &plants {
            if let Ok(Some(reading)) = state.db().latest_reading(plant.id).await {
                readings.push(json!({
                    "plant_id": plant.id,
                    "plant_name": plant.name,
                    "humidity": reading.humidity,
                    "light_lux": reading.light_lux,
                    "luz_ligada": reading.luz_ligada,
                    "read_at": reading.read_at,
                }));
            }
        }

        let payload = json!({ "type": "readings_update", "data": readings });
        let msg = Message::Text(payload.to_string().into());

        if socket.send(msg).await.is_err() {
            // Cliente desconectou
            tracing::debug!("WS: cliente desconectou");
            break;
        }
    }
}
