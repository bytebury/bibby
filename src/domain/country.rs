use crate::infra::geolocation::CountryDetails;
use crate::prelude::*;

#[derive(Debug, Default, Deserialize, Serialize, Clone, FromRow)]
pub struct Country {
    pub id: PrimaryKey,
    pub name: String,
    pub code: String,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Country {
    pub async fn create<'e, E>(exec: E, request: &CountryForm) -> Result<Country>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let country = sqlx::query_as(
            r#"
            INSERT INTO countries (name, code)
            VALUES ($1, LOWER($2))
            RETURNING *
            "#,
        )
        .bind(&request.name)
        .bind(&request.code)
        .fetch_one(exec)
        .await?;
        Ok(country)
    }

    pub async fn find_by_code<'e, E>(exec: E, code: &str) -> Result<Country>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let country = sqlx::query_as("SELECT * FROM countries WHERE code = LOWER($1)")
            .bind(code)
            .fetch_one(exec)
            .await?;
        Ok(country)
    }

    pub async fn search<'e, E>(exec: E, query: &str) -> Result<Vec<Country>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let countries = sqlx::query_as(
            r#"
            SELECT *
            FROM countries
            WHERE name ILIKE '%' || $1 || '%'
               OR code ILIKE '%' || $1 || '%'
            ORDER BY name ASC
            "#,
        )
        .bind(query)
        .fetch_all(exec)
        .await?;
        Ok(countries)
    }

    pub async fn set_locked<'e, E>(exec: E, id: PrimaryKey, locked: bool) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("UPDATE countries SET locked = $1 WHERE id = $2")
            .bind(locked)
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct CountryForm {
    pub name: String,
    pub code: String,
}

impl From<Country> for CountryForm {
    fn from(country: Country) -> Self {
        Self {
            name: country.name,
            code: country.code,
        }
    }
}

impl From<CountryDetails> for CountryForm {
    fn from(details: CountryDetails) -> Self {
        Self {
            name: details.name,
            code: details.code,
        }
    }
}
