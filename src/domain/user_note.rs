use crate::prelude::*;

pub const MAX_USER_NOTE_LEN: usize = 2_000;

/// A private note an admin leaves on an account. Only reachable through
/// `Admin`-guarded routes — the user never sees these.
#[derive(Debug, Clone, FromRow)]
pub struct UserNote {
    pub id: PrimaryKey,
    /// `None` once the authoring admin's account has been deleted.
    pub author_name: Option<String>,
    pub author_image_url: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UserNoteForm {
    pub body: String,
}

impl Validate for UserNoteForm {
    fn validate(&self) -> Result<()> {
        let body = self.body.trim();
        if body.is_empty() {
            return Err(AppError::BadRequest("Note is required.".to_string()));
        }
        if body.chars().count() > MAX_USER_NOTE_LEN {
            return Err(AppError::BadRequest(format!(
                "Note must be {MAX_USER_NOTE_LEN} characters or fewer."
            )));
        }
        Ok(())
    }
}

impl UserNote {
    pub async fn find_by_user_id<'e, E>(exec: E, user_id: PrimaryKey) -> Result<Vec<UserNote>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let notes = sqlx::query_as!(
            UserNote,
            r#"
            SELECT n.id,
                   a.full_name AS "author_name?",
                   a.image_url AS "author_image_url?",
                   n.body,
                   n.created_at
            FROM user_notes n
            LEFT JOIN users a ON a.id = n.author_id
            WHERE n.user_id = $1
            ORDER BY n.created_at DESC, n.id DESC
            "#,
            user_id
        )
        .fetch_all(exec)
        .await?;
        Ok(notes)
    }

    pub async fn create<'e, E>(
        exec: E,
        user_id: PrimaryKey,
        author_id: PrimaryKey,
        form: &UserNoteForm,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            "INSERT INTO user_notes (user_id, author_id, body) VALUES ($1, $2, $3)",
            user_id,
            author_id,
            form.body.trim()
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Scoped to `user_id` so a note id from the URL can only remove a note
    /// that actually belongs to the account whose page it was posted from.
    pub async fn destroy<'e, E>(exec: E, user_id: PrimaryKey, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let result = sqlx::query!(
            "DELETE FROM user_notes WHERE id = $1 AND user_id = $2",
            id,
            user_id
        )
        .execute(exec)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("That note no longer exists.".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(body: &str) -> UserNoteForm {
        UserNoteForm {
            body: body.to_string(),
        }
    }

    #[test]
    fn requires_a_non_blank_body_within_the_limit() {
        assert!(form("Refunded on 9/3").validate().is_ok());
        assert!(form("   ").validate().is_err());
        assert!(form(&"a".repeat(MAX_USER_NOTE_LEN + 1)).validate().is_err());
    }
}
