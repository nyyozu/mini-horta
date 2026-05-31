use chrono::Utc;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use uuid::Uuid;

use crate::models::{
    IrrigationLog, IrrigationTrigger, Plant, SensorReading, User, UserRole,
};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

/// Parseia DateTime tolerando:
/// - RFC3339 padrão:               "2026-05-31T07:47:57+00:00"
/// - RFC3339 com nanoseg. extras:  "2026-05-31T07:47:57.177941400+00:00"
/// - Formato SQLite legado:        "2026-05-31 07:47:57" / "2026-05-31 07:47:57.123456"
fn parse_dt(s: &str) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
    // 1. RFC3339 padrão
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }

    // 2. RFC3339 com nanosegundos extras (>6 dígitos)
    if s.contains('T') {
        let truncated = if let Some(dot) = s.find('.') {
            let end = s[dot + 1..].find(['+', '-', 'Z']).map(|i| dot + 1 + i).unwrap_or(s.len());
            let nanos = &s[dot + 1..end];
            if nanos.len() > 6 {
                format!("{}.{}{}", &s[..dot], &nanos[..6], &s[end..])
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        };
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&truncated) {
            return Ok(dt.with_timezone(&chrono::Utc));
        }
    }

    // 3. Formato SQLite sem timezone: "2026-05-31 07:47:57" ou "2026-05-31 07:47:57.123456"
    let fmt = if s.contains('.') { "%Y-%m-%d %H:%M:%S%.f" } else { "%Y-%m-%d %H:%M:%S" };
    chrono::NaiveDateTime::parse_from_str(s, fmt)
        .map(|ndt| ndt.and_utc())
        .map_err(|e| anyhow::anyhow!("Data inválida '{}': {}", s, e))
}

/// Parseia UUID com ou sem hífens (SQLite às vezes armazena sem hífens).
fn parse_uuid(s: &str) -> anyhow::Result<uuid::Uuid> {
    uuid::Uuid::parse_str(s).or_else(|_| {
        if s.len() == 32 {
            let formatted = format!(
                "{}-{}-{}-{}-{}",
                &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32]
            );
            uuid::Uuid::parse_str(&formatted).map_err(Into::into)
        } else {
            anyhow::bail!("UUID inválido: {s}")
        }
    })
}

impl Database {
    pub async fn connect() -> anyhow::Result<Self> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./horta.db?mode=rwc".to_string());

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&url)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ── Usuários ───────────────────────────────────────────────────────────────

    pub async fn create_user(
        &self,
        email: &str,
        password_hash: &str,
        role: &UserRole,
    ) -> anyhow::Result<User> {
        let id = Uuid::new_v4().to_string();
        let role_str = match role {
            UserRole::Admin => "admin",
            UserRole::User => "user",
        };
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO users (id, email, password_hash, role, created_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING
                id            as "id!",
                email         as "email!",
                password_hash as "password_hash!",
                role          as "role!",
                created_at    as "created_at!"
            "#,
            id, email, password_hash, role_str, now,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(User {
            id: parse_uuid(&row.id)?,
            email: row.email,
            password_hash: row.password_hash,
            role: row.role.parse()?,
            created_at: parse_dt(&row.created_at)?,
        })
    }

    pub async fn find_user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id            as "id!",
                email         as "email!",
                password_hash as "password_hash!",
                role          as "role!",
                created_at    as "created_at!"
            FROM users WHERE email = ?
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| {
            Ok(User {
                id: parse_uuid(&r.id)?,
                email: r.email,
                password_hash: r.password_hash,
                role: r.role.parse()?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    // ── Plantas ────────────────────────────────────────────────────────────────

    pub async fn create_plant(
        &self,
        name: &str,
        description: Option<&str>,
        humidity_min: f64,
        humidity_max: f64,
        created_by: Uuid,
    ) -> anyhow::Result<Plant> {
        let id = Uuid::new_v4().to_string();
        let created_by_str = created_by.to_string();
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO plants (id, name, description, humidity_min, humidity_max, created_by, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            RETURNING
                id          as "id!",
                name        as "name!",
                description,
                humidity_min,
                humidity_max,
                created_by  as "created_by!",
                created_at  as "created_at!"
            "#,
            id, name, description, humidity_min, humidity_max, created_by_str, now
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Plant {
            id: parse_uuid(&row.id)?,
            name: row.name,
            description: row.description,
            humidity_min: row.humidity_min,
            humidity_max: row.humidity_max,
            created_by: parse_uuid(&row.created_by)?,
            created_at: parse_dt(&row.created_at)?,
        })
    }

    pub async fn list_plants(&self) -> anyhow::Result<Vec<Plant>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id          as "id!",
                name        as "name!",
                description,
                humidity_min,
                humidity_max,
                created_by  as "created_by!",
                created_at  as "created_at!"
            FROM plants ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(Plant {
                    id: parse_uuid(&r.id)?,
                    name: r.name,
                    description: r.description,
                    humidity_min: r.humidity_min,
                    humidity_max: r.humidity_max,
                    created_by: parse_uuid(&r.created_by)?,
                    created_at: parse_dt(&r.created_at)?,
                })
            })
            .collect()
    }

    pub async fn get_plant(&self, id: Uuid) -> anyhow::Result<Option<Plant>> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT
                id          as "id!",
                name        as "name!",
                description,
                humidity_min,
                humidity_max,
                created_by  as "created_by!",
                created_at  as "created_at!"
            FROM plants WHERE id = ?
            "#,
            id_str
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| {
            Ok(Plant {
                id: parse_uuid(&r.id)?,
                name: r.name,
                description: r.description,
                humidity_min: r.humidity_min,
                humidity_max: r.humidity_max,
                created_by: parse_uuid(&r.created_by)?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    pub async fn find_plant_by_name(&self, name: &str) -> anyhow::Result<Option<Plant>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id          as "id!",
                name        as "name!",
                description,
                humidity_min,
                humidity_max,
                created_by  as "created_by!",
                created_at  as "created_at!"
            FROM plants WHERE LOWER(name) = LOWER(?)
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| {
            Ok(Plant {
                id: parse_uuid(&r.id)?,
                name: r.name,
                description: r.description,
                humidity_min: r.humidity_min,
                humidity_max: r.humidity_max,
                created_by: parse_uuid(&r.created_by)?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    // ── Leituras de sensor ─────────────────────────────────────────────────────

    pub async fn insert_reading(
        &self,
        plant_id: Uuid,
        humidity: f64,
        light_lux: f64,
    ) -> anyhow::Result<SensorReading> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO sensor_readings (id, plant_id, humidity, light_lux, read_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING
                id       as "id!",
                plant_id as "plant_id!",
                humidity,
                light_lux,
                read_at  as "read_at!"
            "#,
            id, plant_id_str, humidity, light_lux, now
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(SensorReading {
            id: parse_uuid(&row.id)?,
            plant_id: parse_uuid(&row.plant_id)?,
            humidity: row.humidity,
            light_lux: row.light_lux,
            read_at: parse_dt(&row.read_at)?,
        })
    }

    pub async fn latest_reading(&self, plant_id: Uuid) -> anyhow::Result<Option<SensorReading>> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT
                id       as "id!",
                plant_id as "plant_id!",
                humidity,
                light_lux,
                read_at  as "read_at!"
            FROM sensor_readings
            WHERE plant_id = ?
            ORDER BY read_at DESC LIMIT 1
            "#,
            plant_id_str
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| {
            Ok(SensorReading {
                id: parse_uuid(&r.id)?,
                plant_id: parse_uuid(&r.plant_id)?,
                humidity: r.humidity,
                light_lux: r.light_lux,
                read_at: parse_dt(&r.read_at)?,
            })
        })
        .transpose()
    }

    pub async fn list_readings(
        &self,
        plant_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<SensorReading>> {
        let plant_id_str = plant_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                id       as "id!",
                plant_id as "plant_id!",
                humidity,
                light_lux,
                read_at  as "read_at!"
            FROM sensor_readings
            WHERE plant_id = ?
            ORDER BY read_at DESC LIMIT ?
            "#,
            plant_id_str,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(SensorReading {
                    id: parse_uuid(&r.id)?,
                    plant_id: parse_uuid(&r.plant_id)?,
                    humidity: r.humidity,
                    light_lux: r.light_lux,
                    read_at: parse_dt(&r.read_at)?,
                })
            })
            .collect()
    }

    // ── Logs de irrigação ──────────────────────────────────────────────────────

    pub async fn insert_irrigation_log(
        &self,
        plant_id: Uuid,
        trigger: IrrigationTrigger,
        duration_sec: i32,
    ) -> anyhow::Result<IrrigationLog> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let trigger_str = match trigger {
            IrrigationTrigger::Auto => "auto",
            IrrigationTrigger::Manual => "manual",
        };
        let duration_sec = duration_sec as i64;
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO irrigation_logs (id, plant_id, triggered_by, duration_sec, started_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING
                id           as "id!",
                plant_id     as "plant_id!",
                triggered_by as "triggered_by!",
                duration_sec,
                started_at   as "started_at!"
            "#,
            id, plant_id_str, trigger_str, duration_sec, now
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(IrrigationLog {
            id: parse_uuid(&row.id)?,
            plant_id: parse_uuid(&row.plant_id)?,
            triggered_by: row.triggered_by.parse()?,
            duration_sec: row.duration_sec as i32,
            started_at: parse_dt(&row.started_at)?,
        })
    }

    pub async fn list_irrigation_logs(
        &self,
        plant_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<IrrigationLog>> {
        let plant_id_str = plant_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                id           as "id!",
                plant_id     as "plant_id!",
                triggered_by as "triggered_by!",
                duration_sec,
                started_at   as "started_at!"
            FROM irrigation_logs
            WHERE plant_id = ?
            ORDER BY started_at DESC LIMIT ?
            "#,
            plant_id_str,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(IrrigationLog {
                    id: parse_uuid(&r.id)?,
                    plant_id: parse_uuid(&r.plant_id)?,
                    triggered_by: r.triggered_by.parse()?,
                    duration_sec: r.duration_sec as i32,
                    started_at: parse_dt(&r.started_at)?,
                })
            })
            .collect()
    }

    // ── Hortas ─────────────────────────────────────────────────────────────────

    pub async fn create_horta(
        &self,
        code: &str,
        plant_id: Uuid,
        owner_id: Uuid,
    ) -> anyhow::Result<crate::models::Horta> {
        let id         = Uuid::new_v4().to_string();
        let plant_id_s = plant_id.to_string();
        let owner_id_s = owner_id.to_string();
        let now        = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO hortas (id, code, plant_id, owner_id, created_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING
                id         as "id!",
                code       as "code!",
                plant_id   as "plant_id!",
                owner_id   as "owner_id!",
                created_at as "created_at!"
            "#,
            id, code, plant_id_s, owner_id_s, now
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(crate::models::Horta {
            id:         parse_uuid(&row.id)?,
            code:       row.code,
            plant_id:   parse_uuid(&row.plant_id)?,
            owner_id:   parse_uuid(&row.owner_id)?,
            created_at: parse_dt(&row.created_at)?,
        })
    }

    pub async fn find_horta_by_code(&self, code: &str) -> anyhow::Result<Option<crate::models::Horta>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id         as "id!",
                code       as "code!",
                plant_id   as "plant_id!",
                owner_id   as "owner_id!",
                created_at as "created_at!"
            FROM hortas WHERE code = ?
            "#,
            code
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| {
            Ok(crate::models::Horta {
                id:         parse_uuid(&r.id)?,
                code:       r.code,
                plant_id:   parse_uuid(&r.plant_id)?,
                owner_id:   parse_uuid(&r.owner_id)?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    pub async fn list_hortas_by_owner(
        &self,
        owner_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::HortaResponse>> {
        let owner_s = owner_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                h.id         as "id!",
                h.code       as "code!",
                h.owner_id   as "owner_id!",
                h.created_at as "created_at!",
                p.name       as "plant_name!"
            FROM hortas h
            JOIN plants p ON p.id = h.plant_id
            WHERE h.owner_id = ?
            ORDER BY h.created_at DESC
            "#,
            owner_s
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(crate::models::HortaResponse {
                    id:         parse_uuid(&r.id)?,
                    code:       r.code,
                    plant_name: r.plant_name,
                    owner_id:   parse_uuid(&r.owner_id)?,
                    created_at: r.created_at,
                })
            })
            .collect()
    }
}
