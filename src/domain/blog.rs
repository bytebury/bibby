use crate::domain::value_objects::markdown::Markdown;
use crate::infra::markdown;
use crate::infra::pagination::Paginate;
use crate::prelude::*;

#[derive(Debug, Default, Deserialize, Serialize, Clone, FromRow)]
pub struct Blog {
    pub id: PrimaryKey,
    pub user_id: PrimaryKey,
    pub title: String,
    pub content: Markdown,
    pub image_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Blog {
    /// First ~180 chars of the rendered prose (markdown syntax stripped),
    /// broken on a word boundary, with an ellipsis if we truncated. Used as
    /// the card excerpt on the public blog index — readers see "hello", not
    /// "**hello**".
    pub fn excerpt(&self) -> String {
        const LIMIT: usize = 180;
        let plain = markdown::to_plain_text(self.content.raw());
        let trimmed = plain.trim();
        if trimmed.chars().count() <= LIMIT {
            return trimmed.to_string();
        }
        let cut: String = trimmed.chars().take(LIMIT).collect();
        let cut = cut
            .rsplit_once(char::is_whitespace)
            .map(|(a, _)| a)
            .unwrap_or(&cut);
        format!("{}…", cut.trim_end_matches([',', '.', ';', ':']))
    }

    /// "Apr 26, 2026"
    pub fn display_date(&self) -> String {
        self.created_at.format("%b %-d, %Y").to_string()
    }

    /// Rough reading time in minutes, assuming 200 wpm and a 1-minute floor.
    pub fn reading_time(&self) -> i64 {
        let words = self.content.raw().split_whitespace().count() as i64;
        std::cmp::max(1, (words + 199) / 200)
    }

    /// URL-safe slug derived from the title — e.g. "Three Jobs Every…"
    /// becomes "three-jobs-every". Used for `/blogs/{id}/{slug}` SEO URLs.
    /// Always returns at least `"post"` so the route never collapses to
    /// `/blogs/{id}/`.
    pub fn slug(&self) -> String {
        let mut out = String::with_capacity(self.title.len());
        let mut last_dash = true;
        for ch in self.title.chars() {
            if ch.is_ascii_alphanumeric() {
                out.extend(ch.to_lowercase());
                last_dash = false;
            } else if !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
        let trimmed = out.trim_matches('-');
        if trimmed.is_empty() {
            "post".to_string()
        } else {
            trimmed
                .chars()
                .take(80)
                .collect::<String>()
                .trim_end_matches('-')
                .to_string()
        }
    }

    pub async fn create<'e, E>(exec: E, user_id: PrimaryKey, request: &BlogForm) -> Result<Blog>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let blog = sqlx::query_as(
            r#"
            INSERT INTO blogs (user_id, title, content, image_url)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(&request.title)
        .bind(request.content.raw())
        .bind(&request.image_url)
        .fetch_one(exec)
        .await?;
        Ok(blog)
    }

    pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<Blog>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let blog = sqlx::query_as("SELECT * FROM blogs WHERE id = $1")
            .bind(id)
            .fetch_one(exec)
            .await?;
        Ok(blog)
    }

    pub async fn update<'e, E>(
        exec: E,
        blog_id: PrimaryKey,
        user_id: PrimaryKey,
        request: &BlogForm,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            r#"
            UPDATE blogs
            SET user_id = $1, title = $2, content = $3, image_url = $4
            WHERE id = $5
            "#,
        )
        .bind(user_id)
        .bind(&request.title)
        .bind(request.content.raw())
        .bind(&request.image_url)
        .bind(blog_id)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn destroy<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("DELETE FROM blogs WHERE id = $1")
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }
}

impl Paginate for Blog {
    fn table_name() -> &'static str {
        "blogs"
    }
}

#[derive(Deserialize)]
pub struct BlogForm {
    pub title: String,
    pub content: Markdown,
    #[serde(default)]
    pub image_url: String,
}

impl BlogForm {
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(AppError::BadRequest("Title is required.".into()));
        }
        if self.content.is_blank() {
            return Err(AppError::BadRequest("Content is required.".into()));
        }
        Ok(())
    }
}

impl From<Blog> for BlogForm {
    fn from(blog: Blog) -> Self {
        Self {
            title: blog.title,
            content: blog.content,
            image_url: blog.image_url,
        }
    }
}
