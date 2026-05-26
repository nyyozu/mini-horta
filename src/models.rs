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
            "user" => Ok(Self::User),
            other => anyhow::bail!("UserRole inválido: {other:?}"),
        }
    }
}

// ── Usuário ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)] // nunca expõe o hash
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

/// Payload de criação de conta
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: Option<UserRole>, // apenas admins podem definir; padrão = User
}

/// Payload de login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Resposta de login — retorna apenas o token
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
    /// Percentual mínimo de umidade antes de acionar irrigação
    pub humidity_min: f64,
    /// Percentual máximo (para desligar bomba)
    pub humidity_max: f64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlantRequest {
    pub name: String,
    pub description: Option<String>,
    pub humidity_min: f64,
    pub humidity_max: f64,
}

// ── Leitura de sensor ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SensorReading {
    pub id: Uuid,
    pub plant_id: Uuid,
    /// Percentual de umidade do solo (0.0 – 100.0)
    pub humidity: f64,
    /// Luminosidade em lux (>= 0.0)
    pub light_lux: f64,
    pub read_at: DateTime<Utc>,
}

/// Formato JSON enviado pelo Arduino via serial:
/// {"plant_id":"<uuid>","humidity":42.5,"light_lux":850.0}
#[derive(Debug, Deserialize)]
pub struct ArduinoPayload {
    pub plant_id: Uuid,
    pub humidity: f64,
    pub light_lux: f64,
}

impl ArduinoPayload {
    /// Validação de faixa dos valores do sensor
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=100.0).contains(&self.humidity) {
            return Err(format!("Umidade fora de faixa: {}", self.humidity));
        }
        if self.light_lux < 0.0 {
            return Err(format!("Lux inválido: {}", self.light_lux));
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
            "auto" => Ok(Self::Auto),
            "manual" => Ok(Self::Manual),
            other => anyhow::bail!("IrrigationTrigger inválido: {other:?}"),
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
}

#[derive(Debug, Deserialize)]
pub struct ManualIrrigationRequest {
    pub plant_id: Uuid,
    pub duration_sec: i32,
}