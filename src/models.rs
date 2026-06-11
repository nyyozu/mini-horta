use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

// ── Perfis de usuário ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
}

impl FromStr for UserRole {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user"  => Ok(Self::User),
            other   => anyhow::bail!("UserRole inválido: {other:?}"),
        }
    }
}

// ── Usuário ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: Option<UserRole>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub role: UserRole,
}

// ── Planta ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Plant {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub humidity_min: f64,
    pub humidity_max: f64,
    pub luz_horas_dia: f64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlantRequest {
    pub name: String,
    pub description: Option<String>,
    pub humidity_min: f64,
    pub humidity_max: f64,
    #[serde(default = "default_luz_horas")]
    pub luz_horas_dia: f64,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlantRequest {
    pub name: String,
    pub description: Option<String>,
    pub humidity_min: f64,
    pub humidity_max: f64,
    #[serde(default = "default_luz_horas")]
    pub luz_horas_dia: f64,
}

fn default_luz_horas() -> f64 { 12.0 }

// ── Leitura de sensor ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SensorReading {
    pub id: Uuid,
    pub plant_id: Uuid,
    pub humidity: f64,
    /// Segundos totais de luz acumulados no dia
    pub light_lux: f64,
    /// 1 = luz ligada, 0 = luz desligada
    pub luz_ligada: i64,
    pub read_at: DateTime<Utc>,
}

/// Formato JSON enviado pelo Arduino via serial:
/// {"plant_name":"Manjericao","humidity":42.5,"light_lux":3600.0,"luz_ligada":1}
#[derive(Debug, Deserialize)]
pub struct ArduinoPayload {
    pub plant_name: String,
    pub humidity: f64,
    /// Segundos de luz acumulados hoje
    pub light_lux: f64,
    /// 1 = ligada, 0 = desligada
    #[serde(default)]
    pub luz_ligada: i64,
}

impl ArduinoPayload {
    pub fn validate(&self) -> Result<(), String> {
        if self.plant_name.trim().is_empty() {
            return Err("plant_name vazio".to_string());
        }
        if !(0.0..=100.0).contains(&self.humidity) {
            return Err(format!("Umidade fora de faixa: {}", self.humidity));
        }
        if self.light_lux < 0.0 {
            return Err(format!("Segundos de luz inválido: {}", self.light_lux));
        }
        Ok(())
    }
}

// ── Log de irrigação ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IrrigationTrigger {
    Auto,
    Manual,
}

impl FromStr for IrrigationTrigger {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto"   => Ok(Self::Auto),
            "manual" => Ok(Self::Manual),
            other    => anyhow::bail!("IrrigationTrigger inválido: {other:?}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IrrigationLog {
    pub id: Uuid,
    pub plant_id: Uuid,
    pub triggered_by: IrrigationTrigger,
    pub duration_sec: i32,
    pub started_at: DateTime<Utc>,
    /// E-mail de quem disparou. None para irrigações automáticas.
    pub triggered_by_email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManualIrrigationRequest {
    pub plant_id: Uuid,
    pub duration_sec: i32,
}

// ── Horta ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Horta {
    pub id: Uuid,
    pub code: String,
    pub plant_id: Uuid,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HortaResponse {
    pub id: Uuid,
    pub code: String,
    pub plant_name: String,
    pub owner_id: Uuid,
    pub created_at: String,
}
