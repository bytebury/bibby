use crate::prelude::*;

#[derive(Debug, Default, Deserialize, Serialize, Clone, FromRow)]
pub struct City {
    pub id: PrimaryKey,
    pub region_id: PrimaryKey,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl City {
    pub async fn find_or_create<'e, E>(exec: E, region_id: PrimaryKey, name: &str) -> Result<City>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let city = sqlx::query_as!(
            City,
            r#"
            INSERT INTO cities (region_id, name)
            VALUES ($1, $2)
            ON CONFLICT (region_id, name) DO UPDATE SET name = EXCLUDED.name
            RETURNING *
            "#,
            region_id,
            name,
        )
        .fetch_one(exec)
        .await?;
        Ok(city)
    }
}
