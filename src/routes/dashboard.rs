// src/routes/dashboard.rs
// Expõe DashboardResponse, PlantStatus e calculate_status
// para uso pelo hortas.rs via super::dashboard::

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::models::{IrrigationLog, Plant, SensorReading};

// ── Payload de resposta ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DashboardResponse {
    pub plant:               Plant,
    pub latest_reading:      Option<SensorReading>,
    pub recent_logs:         Vec<IrrigationLog>,
    pub status:              PlantStatus,
    pub health_pct:          u8,
    pub luz_total_hoje_seg:  i64,
    /// Status autoritativo de luz da planta — vem de plant_luz_status,
    /// não de sensor_readings, então é correto por planta individual.
    pub luz_ligada:          bool,
    pub fetched_at:          DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantStatus {
    Ok,
    Irrigando,
    SoloUmido,
    SemLeitura,
}

// ── Helper público (usado pelo hortas.rs) ──────────────────────────────────────

pub fn calculate_status(humidity: f64, min: f64, max: f64) -> (PlantStatus, u8) {
    if humidity < min {
        let ratio  = (humidity / min).clamp(0.0, 1.0);
        let health = (ratio * 70.0) as u8;
        (PlantStatus::Irrigando, health)
    } else if humidity > max {
        let ratio  = 1.0 - ((humidity - max) / (100.0 - max)).clamp(0.0, 1.0);
        let health = 70 + (ratio * 30.0) as u8;
        (PlantStatus::SoloUmido, health)
    } else {
        (PlantStatus::Ok, 100)
    }
}
