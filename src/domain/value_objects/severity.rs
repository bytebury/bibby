use crate::prelude::*;

/// Generic visual/intent level — banners, badges, toasts, log lines, etc. all
/// project these onto their own color/icon scheme. Stored as a lowercase TEXT
/// column in Postgres via `sqlx::Type`.
#[derive(Debug, Default, Deserialize, Serialize, sqlx::Type, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase", type_name = "TEXT")]
pub enum Severity {
    Primary,
    #[default]
    Info,
    Warn,
    Danger,
}

impl Display for Severity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Primary => write!(f, "primary"),
            Severity::Info => write!(f, "info"),
            Severity::Warn => write!(f, "warn"),
            Severity::Danger => write!(f, "danger"),
        }
    }
}
