use chrono::Utc;
use rand::Rng;
use rand::distributions::Alphanumeric;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use uuid::Uuid;

use crate::models::{
    IrrigationLog, IrrigationTrigger, LuzLog, Plant, SensorReading, User, UserRole,
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


fn normalize_plant_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'á'|'à'|'â'|'ã'|'ä' => 'a',
            'é'|'è'|'ê'|'ë'     => 'e',
            'í'|'ì'|'î'|'ï'     => 'i',
            'ó'|'ò'|'ô'|'õ'|'ö' => 'o',
            'ú'|'ù'|'û'|'ü'     => 'u',
            'ç'                  => 'c',
            'ñ'                  => 'n',
            other                => other,
        })
        .collect::<String>()
        .to_lowercase()
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

        let db = Self { pool };
        db.seed_system_catalog().await?;
        Ok(db)
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
        luz_horas_dia: f64,
        created_by: Uuid,
        publica: bool,
    ) -> anyhow::Result<Plant> {
        let id = Uuid::new_v4().to_string();
        let created_by_str = created_by.to_string();
        let publica_int: i64 = if publica { 1 } else { 0 };
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO plants (id, name, description, humidity_min, humidity_max, luz_horas_dia, created_by, created_at, publica)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING
                id            as "id!",
                name          as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
            "#,
            id, name, description, humidity_min, humidity_max, luz_horas_dia, created_by_str, now, publica_int
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Plant {
            id: parse_uuid(&row.id)?,
            name: row.name,
            description: row.description,
            humidity_min: row.humidity_min,
            humidity_max: row.humidity_max,
            luz_horas_dia: row.luz_horas_dia,
            created_by: parse_uuid(&row.created_by)?,
            created_at: parse_dt(&row.created_at)?,
        })
    }

    pub async fn list_plants(&self, user_id: Uuid) -> anyhow::Result<Vec<Plant>> {
        let user_id_str = user_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                id            as "id!",
                name          as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
            FROM plants
            WHERE created_by = ?
               OR (publica = 1 AND LOWER(name) NOT IN (
                     SELECT LOWER(name) FROM plants WHERE publica = 0 AND created_by = ?
                   ))
            ORDER BY created_at DESC
            "#,
            user_id_str,
            user_id_str
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
                    luz_horas_dia: r.luz_horas_dia,
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
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
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
                luz_horas_dia: r.luz_horas_dia,
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
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
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
                luz_horas_dia: r.luz_horas_dia,
                created_by: parse_uuid(&r.created_by)?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    /// Busca uma planta pertencente a um usuário específico, por nome
    /// (case-insensitive). Usado para garantir unicidade por usuário
    /// antes de criar uma nova planta privada.
    pub async fn find_plant_by_owner_and_name(
        &self,
        owner_id: Uuid,
        name: &str,
    ) -> anyhow::Result<Option<Plant>> {
        let owner_s = owner_id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT
                id          as "id!",
                name        as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
            FROM plants WHERE created_by = ? AND LOWER(name) = LOWER(?)
            "#,
            owner_s, name
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
                luz_horas_dia: r.luz_horas_dia,
                created_by: parse_uuid(&r.created_by)?,
                created_at: parse_dt(&r.created_at)?,
            })
        })
        .transpose()
    }

    /// Lista plantas visíveis para o usuário, respeitando role:
    /// - Admin vê todas as plantas de todos os usuários.
    /// - Usuário comum vê apenas plantas públicas (sistema/admin) + as suas próprias.
    pub async fn list_plants_for_user(
        &self,
        user_id: Uuid,
        role: &UserRole,
    ) -> anyhow::Result<Vec<Plant>> {
        match role {
            UserRole::Admin => self.list_all_plants().await,
            UserRole::User => self.list_plants(user_id).await,
        }
    }

    /// Garante a existência do usuário sistema (UUID aleatório + senha
    /// aleatória, identificado pela flag is_system) e do catálogo público
    /// de plantas (Manjericão, Salsinha, Hortelã, Alecrim), com UUIDs
    /// aleatórios. Idempotente — roda no boot, não recria nada se já existir.
    pub async fn seed_system_catalog(&self) -> anyhow::Result<()> {
        // 1. Usuário sistema
        let system_id: Uuid = match sqlx::query!(
            r#"SELECT id as "id!" FROM users WHERE is_system = 1 LIMIT 1"#
        )
        .fetch_optional(&self.pool)
        .await?
        {
            Some(row) => parse_uuid(&row.id)?,
            None => {
                let id = Uuid::new_v4();
                let id_str = id.to_string();

                let random_password: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(40)
                    .map(char::from)
                    .collect();
                let hash = bcrypt::hash(&random_password, bcrypt::DEFAULT_COST)?;
                let now = Utc::now().to_rfc3339();

                sqlx::query!(
                    r#"
                    INSERT INTO users (id, email, password_hash, role, created_at, is_system)
                    VALUES (?, 'sistema@horta.local', ?, 'admin', ?, 1)
                    "#,
                    id_str, hash, now
                )
                .execute(&self.pool)
                .await?;

                tracing::info!(user_id = %id, "Usuário sistema criado");
                id
            }
        };

        // 2. Catálogo público
        let catalogo: [(&str, &str, f64, f64, f64); 4] = [
            (
                "Manjericao",
                "Antibacteriano, repelente natural de insetos, auxilia na digestão e é usado em chás calmantes.",
                60.0, 80.0, 12.0,
            ),
            (
                "Salsinha",
                "Rica em vitamina C e K, diurética, antioxidante e amplamente usada como tempero culinário.",
                65.0, 80.0, 12.0,
            ),
            (
                "Hortela",
                "Alivia náuseas e problemas digestivos, descongestionante natural, refrescante em chás e sucos.",
                70.0, 85.0, 8.0,
            ),
            (
                "Alecrim",
                "Estimula memória e concentração, anti-inflamatório, melhora a circulação e é usado em temperos e óleos essenciais.",
                40.0, 60.0, 12.0,
            ),
        ];

        for (name, desc, hmin, hmax, luz) in catalogo {
            let exists = self.find_plant_by_owner_and_name(system_id, name).await?;
            if exists.is_none() {
                self.create_plant(name, Some(desc), hmin, hmax, luz, system_id, true)
                    .await?;
                tracing::info!(plant = name, "Planta de catálogo semeada");
            }
        }

        Ok(())
    }

    // ── Leituras de sensor ─────────────────────────────────────────────────────

    pub async fn insert_reading(
        &self,
        plant_id: Uuid,
        humidity: f64,
        light_lux: f64,
        luz_ligada: i64,
    ) -> anyhow::Result<SensorReading> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO sensor_readings (id, plant_id, humidity, light_lux, luz_ligada, read_at)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING
                id         as "id!",
                plant_id   as "plant_id!",
                humidity,
                light_lux,
                luz_ligada as "luz_ligada!",
                read_at    as "read_at!"
            "#,
            id, plant_id_str, humidity, light_lux, luz_ligada, now
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(SensorReading {
            id: parse_uuid(&row.id)?,
            plant_id: parse_uuid(&row.plant_id)?,
            humidity: row.humidity,
            light_lux: row.light_lux,
            luz_ligada: row.luz_ligada,
            read_at: parse_dt(&row.read_at)?,
        })
    }

    pub async fn latest_reading(&self, plant_id: Uuid) -> anyhow::Result<Option<SensorReading>> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT
                id         as "id!",
                plant_id   as "plant_id!",
                humidity,
                light_lux,
                luz_ligada as "luz_ligada!",
                read_at    as "read_at!"
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
                luz_ligada: r.luz_ligada,
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
                id         as "id!",
                plant_id   as "plant_id!",
                humidity,
                light_lux,
                luz_ligada as "luz_ligada!",
                read_at    as "read_at!"
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
                    luz_ligada: r.luz_ligada,
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
        user_id: Option<Uuid>,
    ) -> anyhow::Result<IrrigationLog> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let user_id_str  = user_id.map(|u| u.to_string());
        let trigger_str = match trigger {
            IrrigationTrigger::Auto => "auto",
            IrrigationTrigger::Manual => "manual",
        };
        let duration_sec_i = duration_sec as i64;
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO irrigation_logs (id, plant_id, triggered_by, duration_sec, started_at, triggered_by_user_id)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING
                id           as "id!",
                plant_id     as "plant_id!",
                triggered_by as "triggered_by!",
                duration_sec,
                started_at   as "started_at!"
            "#,
            id, plant_id_str, trigger_str, duration_sec_i, now, user_id_str
        )
        .fetch_one(&self.pool)
        .await?;

        // Busca o e-mail do usuário se houver
        let email = if let Some(uid) = user_id_str {
            let u = sqlx::query!("SELECT email FROM users WHERE id = ?", uid)
                .fetch_optional(&self.pool).await?;
            u.map(|r| r.email)
        } else {
            None
        };

        Ok(IrrigationLog {
            id: parse_uuid(&row.id)?,
            plant_id: parse_uuid(&row.plant_id)?,
            triggered_by: row.triggered_by.parse()?,
            duration_sec: row.duration_sec as i32,
            started_at: parse_dt(&row.started_at)?,
            triggered_by_email: email,
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
                l.id           as "id!",
                l.plant_id     as "plant_id!",
                l.triggered_by as "triggered_by!",
                l.duration_sec,
                l.started_at   as "started_at!",
                u.email        as "triggered_by_email?"
            FROM irrigation_logs l
            LEFT JOIN users u ON u.id = l.triggered_by_user_id
            WHERE l.plant_id = ?
            ORDER BY l.started_at DESC LIMIT ?
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
                    triggered_by_email: r.triggered_by_email,
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

    /// Lista todas as hortas de todos os usuários, com nome da planta e
    /// e-mail do proprietário. Uso exclusivo do admin.
    pub async fn list_all_hortas_with_owner(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                h.code     as "code!",
                p.name     as "plant_name!",
                u.email    as "owner_email!"
            FROM hortas h
            JOIN plants p ON p.id = h.plant_id
            JOIN users u  ON u.id = h.owner_id
            ORDER BY u.email, h.code
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "code": r.code,
                    "plant_name": r.plant_name,
                    "owner_email": r.owner_email,
                })
            })
            .collect())
    }

    // ── Histórico de luz por planta ────────────────────────────────────────────

    /// Abre um novo período de luz para a planta (luz ligou).
    pub async fn luz_abrir_periodo(&self, plant_id: Uuid) -> anyhow::Result<()> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"INSERT INTO luz_historico (id, plant_id, ligou_em) VALUES (?, ?, ?)"#,
            id, plant_id_str, now
        ).execute(&self.pool).await?;
        Ok(())
    }

    /// Registra uma ação de luz (ligar/desligar) no log, à semelhança de insert_irrigation_log.
    /// `token` é o valor aleatório gerado para plantas simuladas (não-Manjericão); None para Manjericão.
    pub async fn insert_luz_log(
        &self,
        plant_id: Uuid,
        acao: &str,
        token: Option<f64>,
        user_id: Option<Uuid>,
    ) -> anyhow::Result<LuzLog> {
        let id = Uuid::new_v4().to_string();
        let plant_id_str = plant_id.to_string();
        let user_id_str  = user_id.map(|u| u.to_string());
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            INSERT INTO luz_logs (id, plant_id, acao, token, triggered_by_user_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING
                id         as "id!",
                plant_id   as "plant_id!",
                acao       as "acao!",
                token,
                created_at as "created_at!"
            "#,
            id, plant_id_str, acao, token, user_id_str, now
        )
        .fetch_one(&self.pool)
        .await?;

        let email = if let Some(uid) = user_id_str {
            let u = sqlx::query!("SELECT email FROM users WHERE id = ?", uid)
                .fetch_optional(&self.pool).await?;
            u.map(|r| r.email)
        } else {
            None
        };

        Ok(LuzLog {
            id: parse_uuid(&row.id)?,
            plant_id: parse_uuid(&row.plant_id)?,
            acao: row.acao,
            token: row.token,
            created_at: parse_dt(&row.created_at)?,
            triggered_by_email: email,
        })
    }

    /// Lista os últimos logs de ações de luz de uma planta (à semelhança de list_irrigation_logs).
    pub async fn list_luz_logs(
        &self,
        plant_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<LuzLog>> {
        let plant_id_str = plant_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                l.id         as "id!",
                l.plant_id   as "plant_id!",
                l.acao       as "acao!",
                l.token,
                l.created_at as "created_at!",
                u.email      as "triggered_by_email?"
            FROM luz_logs l
            LEFT JOIN users u ON u.id = l.triggered_by_user_id
            WHERE l.plant_id = ?
            ORDER BY l.created_at DESC LIMIT ?
            "#,
            plant_id_str,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(LuzLog {
                    id: parse_uuid(&r.id)?,
                    plant_id: parse_uuid(&r.plant_id)?,
                    acao: r.acao,
                    token: r.token,
                    created_at: parse_dt(&r.created_at)?,
                    triggered_by_email: r.triggered_by_email,
                })
            })
            .collect()
    }

    /// Fecha o período de luz aberto da planta (luz desligou).
    pub async fn luz_fechar_periodo(&self, plant_id: Uuid) -> anyhow::Result<()> {
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            UPDATE luz_historico
            SET desligou_em = ?,
                duracao_sec = CAST((julianday(?) - julianday(ligou_em)) * 86400 AS INTEGER)
            WHERE plant_id = ? AND desligou_em IS NULL
            "#,
            now, now, plant_id_str
        ).execute(&self.pool).await?;
        Ok(())
    }

    /// Fecha todos os períodos abertos de todas as plantas (chamado à meia-noite).
    pub async fn luz_fechar_todos_periodos(&self) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            UPDATE luz_historico
            SET desligou_em = ?,
                duracao_sec = CAST((julianday(?) - julianday(ligou_em)) * 86400 AS INTEGER)
            WHERE desligou_em IS NULL
            "#,
            now, now
        ).execute(&self.pool).await?;
        tracing::info!("Meia-noite: todos os períodos de luz fechados");
        Ok(())
    }

    /// Total de segundos de luz recebida hoje pela planta.
    pub async fn luz_total_hoje(&self, plant_id: Uuid) -> anyhow::Result<i64> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(
                CASE
                    WHEN desligou_em IS NOT NULL THEN duracao_sec
                    ELSE CAST((julianday('now') - julianday(ligou_em)) * 86400 AS INTEGER)
                END
            ), 0) as "total!"
            FROM luz_historico
            WHERE plant_id = ?
              AND date(ligou_em) = date('now')
            "#,
            plant_id_str
        ).fetch_one(&self.pool).await?;
        Ok(row.total.into())
    }

    /// Histórico dos últimos N dias de luz da planta.
    /// Retorna uma linha por dia com o total de segundos acumulados.
    pub async fn luz_historico_dias(&self, plant_id: Uuid, dias: i64) -> anyhow::Result<Vec<serde_json::Value>> {
        let plant_id_str = plant_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                date(ligou_em)  as "dia!",
                SUM(CASE
                    WHEN desligou_em IS NOT NULL THEN duracao_sec
                    ELSE CAST((julianday('now') - julianday(ligou_em)) * 86400 AS INTEGER)
                END)            as "total_sec!"
            FROM luz_historico
            WHERE plant_id = ?
              AND ligou_em >= datetime('now', '-' || ? || ' days')
            GROUP BY date(ligou_em)
            ORDER BY date(ligou_em) DESC
            "#,
            plant_id_str, dias
        ).fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|r| serde_json::json!({
            "dia":       r.dia,
            "total_sec": r.total_sec,
        })).collect())
    }

    /// Lista todas as plantas sem filtro — uso interno (WS, serial, buscas por nome).
    /// Retorna todas as plantas com metadados extras para o painel admin:
    /// - `publica`: true = catálogo do sistema, false = criada por usuário
    /// - `owner_email`: e-mail de quem criou
    pub async fn list_all_plants_with_meta(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                p.id            as "id!",
                p.name          as "name!",
                p.description,
                p.humidity_min,
                p.humidity_max,
                p.luz_horas_dia as "luz_horas_dia!",
                p.created_by    as "created_by!",
                p.created_at    as "created_at!",
                p.publica       as "publica!",
                u.email         as "owner_email!"
            FROM plants p
            JOIN users u ON u.id = p.created_by
            ORDER BY p.publica DESC, p.created_at ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| serde_json::json!({
            "id":           r.id,
            "name":         r.name,
            "description":  r.description,
            "humidity_min": r.humidity_min,
            "humidity_max": r.humidity_max,
            "luz_horas_dia": r.luz_horas_dia,
            "created_by":   r.created_by,
            "created_at":   r.created_at,
            "publica":      r.publica == 1,
            "owner_email":  r.owner_email,
        })).collect())
    }

        pub async fn list_all_plants(&self) -> anyhow::Result<Vec<Plant>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id            as "id!",
                name          as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
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
                    luz_horas_dia: r.luz_horas_dia,
                    created_by: parse_uuid(&r.created_by)?,
                    created_at: parse_dt(&r.created_at)?,
                })
            })
            .collect()
    }
    /// Permite encontrar "Manjericao" e "Manjericão" como a mesma planta.
    pub async fn find_plant_by_normalized_name(&self, normalized: &str) -> anyhow::Result<Option<Plant>> {
        // Busca todas as plantas e filtra pelo nome normalizado em memória
        // (SQLite não tem funções de normalização Unicode nativas)
        let plants = self.list_all_plants().await?;
        let found = plants.into_iter().find(|p| {
            normalize_plant_name(&p.name) == normalized
        });
        Ok(found)
    }

    /// Busca uma planta privada (publica = 0) pertencente ao owner_id, pelo nome normalizado.
    /// Usado para reutilizar a instância já existente do usuário sem criar duplicata.
    pub async fn find_owned_plant_by_normalized_name(
        &self,
        normalized: &str,
        owner_id: Uuid,
    ) -> anyhow::Result<Option<Plant>> {
        let owner_str = owner_id.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT
                id            as "id!",
                name          as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
            FROM plants
            WHERE publica = 0 AND created_by = ?
            "#,
            owner_str
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .find(|r| normalize_plant_name(&r.name) == normalized)
            .map(|r| {
                Ok(Plant {
                    id: parse_uuid(&r.id)?,
                    name: r.name,
                    description: r.description,
                    humidity_min: r.humidity_min,
                    humidity_max: r.humidity_max,
                    luz_horas_dia: r.luz_horas_dia,
                    created_by: parse_uuid(&r.created_by)?,
                    created_at: parse_dt(&r.created_at)?,
                })
            })
            .transpose()
    }

    /// Inicializa o histórico de luz com um período já fechado e duração aleatória,
    /// apenas se não houver nenhum registro ainda para a planta.
    /// Duração entre 1s e luz_horas_dia * 3600s da planta.
    pub async fn init_luz_historico(&self, plant_id: Uuid) -> anyhow::Result<()> {
        let plant_id_str = plant_id.to_string();

        // Verifica se já existe algum registro
        let count = sqlx::query!(
            r#"SELECT COUNT(*) as "n!" FROM luz_historico WHERE plant_id = ?"#,
            plant_id_str
        )
        .fetch_one(&self.pool)
        .await?;

        if count.n > 0 {
            return Ok(());
        }

        // Busca a meta de luz da planta (luz_horas_dia)
        let plant = sqlx::query!(
            r#"SELECT luz_horas_dia as "luz_horas_dia!" FROM plants WHERE id = ?"#,
            plant_id_str
        )
        .fetch_optional(&self.pool)
        .await?;

        let meta_seg = plant.map(|p| (p.luz_horas_dia * 3600.0) as u64).unwrap_or(36000);

        // Duração aleatória entre 1s e meta_seg
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        let duracao_sec = if meta_seg > 1 { 1 + (seed % (meta_seg - 1)) } else { 1 };
        let duracao_sec = duracao_sec as i64;

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let ligou_em    = (now - chrono::Duration::seconds(duracao_sec)).to_rfc3339();
        let desligou_em = now.to_rfc3339();

        sqlx::query!(
            r#"INSERT INTO luz_historico (id, plant_id, ligou_em, desligou_em, duracao_sec)
               VALUES (?, ?, ?, ?, ?)"#,
            id, plant_id_str, ligou_em, desligou_em, duracao_sec
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Atualiza a planta associada a uma horta.
    pub async fn update_horta_plant(&self, horta_id: Uuid, plant_id: Uuid) -> anyhow::Result<()> {
        let horta_id_str = horta_id.to_string();
        let plant_id_str = plant_id.to_string();
        sqlx::query!(
            "UPDATE hortas SET plant_id = ? WHERE id = ?",
            plant_id_str, horta_id_str
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Retorna true se a luz desta planta está ligada.
    /// Usa a tabela plant_luz_status — independente das leituras do Arduino.
    pub async fn get_luz_status(&self, plant_id: Uuid) -> anyhow::Result<bool> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"SELECT ligada FROM plant_luz_status WHERE plant_id = ?"#,
            plant_id_str
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.ligada == 1).unwrap_or(false))
    }

    /// Grava o estado de luz de uma planta (ligada/desligada).
    /// Faz UPSERT — cria o registro se não existir.
    pub async fn set_luz_status(&self, plant_id: Uuid, ligada: bool) -> anyhow::Result<()> {
        let plant_id_str = plant_id.to_string();
        let valor: i64   = if ligada { 1 } else { 0 };
        let now          = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            INSERT INTO plant_luz_status (plant_id, ligada, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(plant_id) DO UPDATE SET ligada = excluded.ligada, updated_at = excluded.updated_at
            "#,
            plant_id_str, valor, now
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Compatibilidade: atualiza luz_ligada na sensor_reading mais recente.
    /// Mantido para não quebrar código existente, mas o estado autoritativo
    /// agora está em plant_luz_status.
    pub async fn set_luz_ligada(&self, plant_id: Uuid, ligada: bool) -> anyhow::Result<()> {
        // Delega para set_luz_status (fonte de verdade)
        self.set_luz_status(plant_id, ligada).await?;

        // Abre ou fecha o período de histórico em sincronia com o status,
        // garantindo que luz_total_hoje nunca fique contabilizando com luz desligada.
        if ligada {
            self.luz_abrir_periodo(plant_id).await?;
        } else {
            self.luz_fechar_periodo(plant_id).await?;
        }

        // Atualiza também a leitura mais recente para manter consistência no histórico
        let plant_id_str = plant_id.to_string();
        let valor: i64   = if ligada { 1 } else { 0 };
        sqlx::query!(
            r#"UPDATE sensor_readings SET luz_ligada = ?
               WHERE id = (SELECT id FROM sensor_readings WHERE plant_id = ? ORDER BY read_at DESC LIMIT 1)"#,
            valor, plant_id_str
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Status de umidade por planta ──────────────────────────────────────────

    /// Retorna a umidade atual da planta e quando foi atualizada pela última vez.
    pub async fn get_umidade_status_com_tempo(&self, plant_id: Uuid) -> anyhow::Result<Option<(f64, chrono::DateTime<chrono::Utc>)>> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"SELECT umidade, updated_at as "updated_at!" FROM plant_umidade_status WHERE plant_id = ?"#,
            plant_id_str
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| parse_dt(&r.updated_at).map(|t| (r.umidade, t)))
            .transpose()
    }

    /// Retorna a umidade atual da planta.
    /// Se não existir registro ainda, retorna None.
    pub async fn get_umidade_status(&self, plant_id: Uuid) -> anyhow::Result<Option<f64>> {
        let plant_id_str = plant_id.to_string();
        let row = sqlx::query!(
            r#"SELECT umidade FROM plant_umidade_status WHERE plant_id = ?"#,
            plant_id_str
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.umidade))
    }

    /// Grava a umidade atual de uma planta. Faz UPSERT.
    pub async fn set_umidade_status(&self, plant_id: Uuid, umidade: f64) -> anyhow::Result<()> {
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            INSERT INTO plant_umidade_status (plant_id, umidade, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(plant_id) DO UPDATE SET umidade = excluded.umidade, updated_at = excluded.updated_at
            "#,
            plant_id_str, umidade, now
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Inicializa a umidade de uma planta com o meio do range (humidity_min + humidity_max) / 2,
    /// apenas se ainda não existir registro. Chamado quando a horta é conectada.
    pub async fn init_umidade_status(&self, plant_id: Uuid, humidity_min: f64, humidity_max: f64) -> anyhow::Result<()> {
        let plant_id_str = plant_id.to_string();
        let now = Utc::now().to_rfc3339();
        let umidade_inicial = (humidity_min + humidity_max) / 2.0;
        sqlx::query!(
            r#"
            INSERT INTO plant_umidade_status (plant_id, umidade, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(plant_id) DO NOTHING
            "#,
            plant_id_str, umidade_inicial, now
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Cooldown de luz por horta ──────────────────────────────────────────────

    /// Retorna o timestamp Unix do último comando de luz enviado para esta horta.
    pub async fn get_luz_cooldown(&self, horta_id: Uuid) -> anyhow::Result<Option<i64>> {
        let horta_id_str = horta_id.to_string();
        let row = sqlx::query!(
            r#"SELECT ultimo_comando_at FROM luz_cooldown WHERE horta_id = ?"#,
            horta_id_str
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.ultimo_comando_at))
    }

    /// Grava (ou atualiza) o timestamp do último comando de luz desta horta.
    pub async fn set_luz_cooldown(&self, horta_id: Uuid) -> anyhow::Result<()> {
        let horta_id_str = horta_id.to_string();
        let now_ts = Utc::now().timestamp();
        sqlx::query!(
            r#"
            INSERT INTO luz_cooldown (horta_id, ultimo_comando_at)
            VALUES (?, ?)
            ON CONFLICT(horta_id) DO UPDATE SET ultimo_comando_at = excluded.ultimo_comando_at
            "#,
            horta_id_str, now_ts
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_plant(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        humidity_min: f64,
        humidity_max: f64,
        luz_horas_dia: f64,
    ) -> anyhow::Result<Plant> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            r#"
            UPDATE plants
            SET name = ?, description = ?, humidity_min = ?, humidity_max = ?, luz_horas_dia = ?
            WHERE id = ?
            RETURNING
                id            as "id!",
                name          as "name!",
                description,
                humidity_min,
                humidity_max,
                luz_horas_dia as "luz_horas_dia!",
                created_by    as "created_by!",
                created_at    as "created_at!"
            "#,
            name, description, humidity_min, humidity_max, luz_horas_dia, id_str
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Plant {
            id: parse_uuid(&row.id)?,
            name: row.name,
            description: row.description,
            humidity_min: row.humidity_min,
            humidity_max: row.humidity_max,
            luz_horas_dia: row.luz_horas_dia,
            created_by: parse_uuid(&row.created_by)?,
            created_at: parse_dt(&row.created_at)?,
        })
    }

    pub async fn delete_plant(&self, id: Uuid) -> anyhow::Result<()> {
        let id_str = id.to_string();
        sqlx::query!("DELETE FROM plants WHERE id = ?", id_str)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

}