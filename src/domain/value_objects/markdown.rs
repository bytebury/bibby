use crate::infra::markdown::render;
use crate::prelude::*;

/// CommonMark string. Persists to Postgres as `TEXT` (via `sqlx::Type`
/// transparency) and (de)serializes as a plain string for forms/JSON.
///
/// In askama templates:
/// - `{{ field|safe }}` renders the field as sanitized HTML (`Display` calls
///   `infra::markdown::render`, which goes through `ammonia` for safety).
/// - `{{ field.raw() }}` shows the source verbatim — use for textareas and
///   admin previews where the author wants to see what they wrote.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Markdown(pub String);

impl Markdown {
    pub fn raw(&self) -> &str {
        &self.0
    }

    /// Whether the source — ignoring leading/trailing whitespace — is empty.
    /// Useful for form validation.
    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl Display for Markdown {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&render(&self.0))
    }
}

impl From<String> for Markdown {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Markdown {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
