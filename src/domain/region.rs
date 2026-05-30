use crate::prelude::*;

#[derive(Debug, Default, Deserialize, Serialize, Clone, FromRow)]
pub struct Region {
    pub id: PrimaryKey,
    pub country_id: PrimaryKey,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Region {
    pub async fn find_or_create<'e, E>(
        exec: E,
        country_id: PrimaryKey,
        name: &str,
    ) -> Result<Region>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let region = sqlx::query_as(
            r#"
            INSERT INTO regions (country_id, name)
            VALUES ($1, $2)
            ON CONFLICT (country_id, name) DO UPDATE SET name = EXCLUDED.name
            RETURNING *
            "#,
        )
        .bind(country_id)
        .bind(name)
        .fetch_one(exec)
        .await?;
        Ok(region)
    }
}
