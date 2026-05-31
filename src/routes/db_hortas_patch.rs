// Adicione em src/models.rs:

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Horta {
    pub id:         Uuid,
    pub code:       String,
    pub plant_id:   Uuid,
    pub owner_id:   Uuid,
    pub created_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────
// Adicione estes métodos ao impl Database em src/db.rs:

pub async fn create_horta(
    &self,
    code: &str,
    plant_id: Uuid,
    owner_id: Uuid,
) -> anyhow::Result<Horta> {
    let id          = Uuid::new_v4().to_string();
    let plant_id_s  = plant_id.to_string();
    let owner_id_s  = owner_id.to_string();
    let now         = Utc::now().to_rfc3339();

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

    Ok(Horta {
        id:         Uuid::parse_str(&row.id)?,
        code:       row.code,
        plant_id:   Uuid::parse_str(&row.plant_id)?,
        owner_id:   Uuid::parse_str(&row.owner_id)?,
        created_at: row.created_at.parse()?,
    })
}

pub async fn find_horta_by_code(&self, code: &str) -> anyhow::Result<Option<Horta>> {
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

    row.map(|r| Ok(Horta {
        id:         Uuid::parse_str(&r.id)?,
        code:       r.code,
        plant_id:   Uuid::parse_str(&r.plant_id)?,
        owner_id:   Uuid::parse_str(&r.owner_id)?,
        created_at: r.created_at.parse()?,
    }))
    .transpose()
}

pub async fn list_hortas_by_owner(&self, owner_id: Uuid) -> anyhow::Result<Vec<crate::routes::hortas::HortaResponse>> {
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

    rows.into_iter().map(|r| Ok(crate::routes::hortas::HortaResponse {
        id:         Uuid::parse_str(&r.id)?,
        code:       r.code,
        plant_name: r.plant_name,
        owner_id:   Uuid::parse_str(&r.owner_id)?,
        created_at: r.created_at,
    })).collect()
}
