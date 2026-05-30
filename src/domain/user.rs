use crate::domain::Access;
use crate::infra::pagination::Paginate;
use crate::prelude::*;

#[derive(Debug, Deserialize, Serialize, Clone, FromRow)]
pub struct User {
    pub id: PrimaryKey,
    pub country_id: Option<PrimaryKey>,
    pub region_id: Option<PrimaryKey>,
    pub full_name: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: String,
    pub image_url: String,
    pub role: Role,
    pub verified: bool,
    pub locked: bool,
    pub last_known_ip: String,
    /// `country_code` and `country_name` come from the `user_details` view's
    /// LEFT JOIN on countries, COALESCEd to `"unknown"` / `"Unknown"` so the
    /// template can always render `/assets/images/flags/{code}.svg`.
    pub country_code: String,
    pub country_name: String,
    pub region_name: Option<String>,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub async fn create<'e, E>(exec: E, request: &CreateUser) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres> + Clone,
    {
        let user_id: PrimaryKey = sqlx::query_scalar(
            r#"
            INSERT INTO users (full_name, first_name, last_name, email, image_url, role, verified, locked, last_known_ip, country_id, region_id)
            VALUES ($1, $2, $3, $4, $5, COALESCE((SELECT 'admin' WHERE NOT EXISTS (SELECT 1 FROM users)), $6), $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(&request.full_name)
        .bind(&request.first_name)
        .bind(&request.last_name)
        .bind(&request.email)
        .bind(&request.image_url)
        .bind(request.role.to_string())
        .bind(request.verified)
        .bind(request.locked)
        .bind(&request.last_known_ip)
        .bind(request.country_id)
        .bind(request.region_id)
        .fetch_one(exec.clone())
        .await?;

        User::find_by_id(exec, user_id).await
    }

    pub async fn find_by_email<'e, E>(exec: E, email: &str) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as("SELECT * FROM user_details WHERE email = $1")
            .bind(email)
            .fetch_one(exec)
            .await?;
        Ok(user)
    }

    pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as("SELECT * FROM user_details WHERE id = $1")
            .bind(id)
            .fetch_one(exec)
            .await?;
        Ok(user)
    }

    pub async fn update<'e, E>(exec: E, id: PrimaryKey, request: &UserForm) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres> + Clone,
    {
        sqlx::query(
            r#"
            UPDATE users
            SET role = $1, locked = $2, last_known_ip = $3, last_seen_at = $4, country_id = $5, region_id = $6
            WHERE id = $7
            "#,
        )
        .bind(request.role.to_string())
        .bind(request.locked.is_some())
        .bind(&request.last_known_ip)
        .bind(request.last_seen_at)
        .bind(request.country_id)
        .bind(request.region_id)
        .bind(id)
        .execute(exec.clone())
        .await?;

        User::find_by_id(exec, id).await
    }

    pub async fn destroy<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, Role::Admin)
    }

    pub fn is_free(&self) -> bool {
        matches!(self.role, Role::Free)
    }

    pub fn ensure_can_read<T>(&self, entity: &T) -> Result<()>
    where
        Self: Access<T>,
    {
        if self.can_read(entity) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden".to_string()))
        }
    }

    pub fn ensure_can_write<T>(&self, entity: &T) -> Result<()>
    where
        Self: Access<T>,
    {
        if self.can_write(entity) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden".to_string()))
        }
    }
}

impl Access<User> for User {
    fn can_read(&self, entity: &User) -> bool {
        self.is_admin() || self.id == entity.id
    }
    fn can_write(&self, _: &User) -> bool {
        self.is_admin()
    }
}

impl Paginate for User {
    fn table_name() -> &'static str {
        "user_details"
    }
}

#[derive(Clone, Debug)]
pub struct CreateUser {
    pub country_id: Option<PrimaryKey>,
    pub region_id: Option<PrimaryKey>,
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub full_name: String,
    pub image_url: String,
    pub role: Role,
    pub verified: bool,
    pub locked: bool,
    pub last_known_ip: String,
}

#[derive(Debug, Default, Deserialize, Serialize, sqlx::Type, Clone, PartialOrd, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case", type_name = "TEXT")]
pub enum Role {
    #[default]
    Free,
    Professional,
    Admin,
}

impl From<String> for Role {
    fn from(value: String) -> Self {
        match value.as_str() {
            "admin" => Role::Admin,
            "professional" => Role::Professional,
            _ => Role::Free,
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Professional => write!(f, "professional"),
            Role::Free => write!(f, "free"),
        }
    }
}

#[derive(Deserialize)]
pub struct UserForm {
    pub role: Role,
    pub locked: Option<String>,
    pub country_id: Option<PrimaryKey>,
    pub region_id: Option<PrimaryKey>,
    pub last_known_ip: String,
    pub last_seen_at: DateTime<Utc>,
}

impl From<User> for UserForm {
    fn from(user: User) -> Self {
        Self {
            role: user.role,
            locked: user.locked.then_some("on".into()),
            country_id: user.country_id,
            region_id: user.region_id,
            last_seen_at: user.last_seen_at,
            last_known_ip: user.last_known_ip,
        }
    }
}
