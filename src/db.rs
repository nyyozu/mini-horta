use chrono::Utc;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use uuid::Uuid;

use crate::models::{
    IrrigationLog, IrrigationTrigger, Plant, SensorReading, User, UserRole,
};

/// Wrapper em torno do pool SQLx.
/// Troca SqlitePool por PgPool bastando mudar este alias e DATABASE_URL.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Conecta ao banco via DATABASE_URL e aplica migrations da pasta ./migrations/
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
        // FIX 1: era `i64::new_v4()` — UUID não tem nada a ver com i64.
        let id = Uuid::new_v4().to_string();
        let role_str = match role {
            UserRole::Admin => "admin",
            UserRole::User => "user",
        };
        let now = Utc::now().to_rfc3339();

        // FIX 2: SQLite armazena UUID como TEXT; query! retorna String,
        // então convertemos manualmente em vez de depender do cast `as "id: Uuid"`.
        let row = sqlx::query!(
            r#"
            INSERT INTO users (id, email, password_hash, role, created_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING
                id        as "id!",
                email     as "email!",
                password_hash as "password_hash!",
                role      as "role!",
                created_at as "created_at!"
            "#,
            id,
            email,
            password_hash,
            role_str,
            now,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(User {
            id: Uuid::parse_str(&row.id)?,
            email: row.email,
            password_hash: row.password_hash,
            role: row.role.parse()?,
            created_at: row.created_at.parse()?,
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
                id: Uuid::parse_str(&r.id)?,
                email: r.email,
                password_hash: r.password_hash,
                role: r.role.parse()?,
                created_at: r.created_at.parse()?,
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
        // FIX 2: Uuid deve ser serializado como String para SQLite
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
            id: Uuid::parse_str(&row.id)?,
            name: row.name,
            description: row.description,
            humidity_min: row.humidity_min,
            humidity_max: row.humidity_max,
            created_by: Uuid::parse_str(&row.created_by)?,
            created_at: row.created_at.parse()?,
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
                    id: Uuid::parse_str(&r.id)?,
                    name: r.name,
                    description: r.description,
                    humidity_min: r.humidity_min,
                    humidity_max: r.humidity_max,
                    created_by: Uuid::parse_str(&r.created_by)?,
                    created_at: r.created_at.parse()?,
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
                id: Uuid::parse_str(&r.id)?,
                name: r.name,
                description: r.description,
                humidity_min: r.humidity_min,
                humidity_max: r.humidity_max,
                created_by: Uuid::parse_str(&r.created_by)?,
                created_at: r.created_at.parse()?,
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
            id: Uuid::parse_str(&row.id)?,
            plant_id: Uuid::parse_str(&row.plant_id)?,
            humidity: row.humidity,
            light_lux: row.light_lux,
            read_at: row.read_at.parse()?,
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
                id: Uuid::parse_str(&r.id)?,
                plant_id: Uuid::parse_str(&r.plant_id)?,
                humidity: r.humidity,
                light_lux: r.light_lux,
                read_at: r.read_at.parse()?,
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
                    id: Uuid::parse_str(&r.id)?,
                    plant_id: Uuid::parse_str(&r.plant_id)?,
                    humidity: r.humidity,
                    light_lux: r.light_lux,
                    read_at: r.read_at.parse()?,
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
        // FIX 2: SQLite só conhece i64 para INTEGER — cast explícito aqui
        // evita o erro "expected i64, found i32" que o sqlx emite em compile time.
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
            id: Uuid::parse_str(&row.id)?,
            plant_id: Uuid::parse_str(&row.plant_id)?,
            triggered_by: row.triggered_by.parse()?,
            // FIX 2: volta para i32 se o modelo exige — o banco entrega i64
            duration_sec: row.duration_sec as i32,
            started_at: row.started_at.parse()?,
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
                    id: Uuid::parse_str(&r.id)?,
                    plant_id: Uuid::parse_str(&r.plant_id)?,
                    triggered_by: r.triggered_by.parse()?,
                    duration_sec: r.duration_sec as i32,
                    started_at: r.started_at.parse()?,
                })
            })
            .collect()
    }
}