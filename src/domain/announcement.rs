use crate::domain::value_objects::markdown::Markdown;
use crate::domain::value_objects::severity::Severity;
use crate::infra::pagination::Paginate;
use crate::prelude::*;

#[derive(Debug, Default, Deserialize, Serialize, Clone, FromRow)]
pub struct Announcement {
    pub id: PrimaryKey,
    pub user_id: PrimaryKey,
    pub title: String,
    pub message: Markdown,
    pub active: bool,
    pub severity: Severity,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Announcement {
    pub async fn create<'e, E>(
        exec: E,
        user_id: PrimaryKey,
        request: &AnnouncementForm,
    ) -> Result<Announcement>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query_as!(
            Announcement,
            r#"
            INSERT INTO announcements (user_id, title, message, active, severity)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                user_id,
                title,
                message as "message: Markdown",
                active,
                severity as "severity: Severity",
                created_at,
                updated_at
            "#,
            user_id,
            request.title,
            request.message.raw(),
            request.active.is_some(),
            request.severity.to_string(),
        )
        .fetch_one(exec)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<Announcement>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query_as!(
            Announcement,
            r#"
            SELECT
                id,
                user_id,
                title,
                message as "message: Markdown",
                active,
                severity as "severity: Severity",
                created_at,
                updated_at
            FROM announcements WHERE id = $1
            "#,
            id,
        )
        .fetch_one(exec)
        .await?;
        Ok(row)
    }

    /// The single banner shown sitewide: latest active announcement, if any.
    pub async fn find_latest_active<'e, E>(exec: E) -> Result<Option<Announcement>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query_as!(
            Announcement,
            r#"
            SELECT
                id,
                user_id,
                title,
                message as "message: Markdown",
                active,
                severity as "severity: Severity",
                created_at,
                updated_at
            FROM announcements
            WHERE active = true
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(exec)
        .await?;
        Ok(row)
    }

    pub async fn update<'e, E>(
        exec: E,
        id: PrimaryKey,
        user_id: PrimaryKey,
        request: &AnnouncementForm,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            r#"
            UPDATE announcements
            SET user_id = $1, title = $2, message = $3, active = $4, severity = $5
            WHERE id = $6
            "#,
        )
        .bind(user_id)
        .bind(&request.title)
        .bind(request.message.raw())
        .bind(request.active.is_some())
        .bind(request.severity.to_string())
        .bind(id)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn set_active<'e, E>(exec: E, id: PrimaryKey, active: bool) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("UPDATE announcements SET active = $1 WHERE id = $2")
            .bind(active)
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub async fn destroy<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("DELETE FROM announcements WHERE id = $1")
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }
}

impl Paginate for Announcement {
    fn table_name() -> &'static str {
        "announcements"
    }
}

#[derive(Deserialize)]
pub struct AnnouncementForm {
    pub title: String,
    pub message: Markdown,
    pub active: Option<String>,
    #[serde(default)]
    pub severity: Severity,
}

impl AnnouncementForm {
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(AppError::BadRequest("Title is required.".into()));
        }
        if self.message.is_blank() {
            return Err(AppError::BadRequest("Message is required.".into()));
        }
        Ok(())
    }
}

impl From<Announcement> for AnnouncementForm {
    fn from(announcement: Announcement) -> Self {
        Self {
            title: announcement.title,
            message: announcement.message,
            active: announcement.active.then_some("on".into()),
            severity: announcement.severity,
        }
    }
}
