use crate::domain::user::User;
use crate::infra::db::SharedDatabase;
use crate::infra::pagination::{Paginated, PagingInfo};
use crate::paginate_with;
use crate::prelude::*;

/// Plan filter on the admin Manage Users list.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RoleFilter {
    #[default]
    All,
    Free,
    Professional,
    Admin,
}

impl RoleFilter {
    pub const ALL: [RoleFilter; 4] = [
        RoleFilter::All,
        RoleFilter::Free,
        RoleFilter::Professional,
        RoleFilter::Admin,
    ];

    /// Unknown values fall back to `All` rather than failing the request.
    pub fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "free" => Self::Free,
            "professional" => Self::Professional,
            "admin" => Self::Admin,
            _ => Self::All,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "",
            Self::Free => "free",
            Self::Professional => "professional",
            Self::Admin => "admin",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Free => "Free",
            Self::Professional => "Professional",
            Self::Admin => "Admin",
        }
    }

    /// Fixed SQL fragments only — nothing from the request is interpolated.
    /// Mirrors `Role::from(String)`: anything that isn't a paid or admin role
    /// is Free.
    fn sql(&self) -> &'static str {
        match self {
            Self::All => "TRUE",
            Self::Free => "role NOT IN ('admin', 'professional')",
            Self::Professional => "role = 'professional'",
            Self::Admin => "role = 'admin'",
        }
    }
}

pub struct SearchUseCase {
    db: SharedDatabase,
}

impl SearchUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    /// Matches `filter` against email, name, and IP. `flagged_only` narrows to
    /// accounts an admin has flagged; it combines with the role filter.
    pub async fn execute(
        &self,
        filter: &str,
        role: RoleFilter,
        flagged_only: bool,
        paging_info: &PagingInfo,
    ) -> Result<Paginated<User>> {
        let flagged = if flagged_only {
            "id IN (SELECT user_id FROM user_flags)"
        } else {
            "TRUE"
        };
        let where_clause = format!(
            "WHERE (email ILIKE '%' || $1 || '%' \
                    OR full_name ILIKE '%' || $2 || '%' \
                    OR last_known_ip LIKE '%' || $3 || '%') \
               AND {} AND {flagged} \
             ORDER BY last_seen_at DESC",
            role.sql()
        );
        paginate_with!(
            User,
            &self.db,
            &where_clause,
            vec![filter, filter, filter],
            paging_info
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_filters_and_falls_back_to_all() {
        for filter in RoleFilter::ALL {
            assert_eq!(RoleFilter::parse(Some(filter.as_str())), filter);
        }
        assert_eq!(RoleFilter::parse(Some("bogus")), RoleFilter::All);
        assert_eq!(RoleFilter::parse(None), RoleFilter::All);
    }
}
