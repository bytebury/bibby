use crate::domain::user::User;
use crate::infra::db::SharedDatabase;
use crate::infra::pagination::{Paginated, PagingInfo};
use crate::paginate_with;
use crate::prelude::*;

pub struct SearchUseCase {
    db: SharedDatabase,
}

impl SearchUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    pub async fn execute(&self, filter: &str, paging_info: &PagingInfo) -> Result<Paginated<User>> {
        paginate_with!(
            User,
            &self.db,
            "WHERE email ILIKE '%' || $1 || '%' \
                    OR full_name ILIKE '%' || $2 || '%' \
                    OR last_known_ip LIKE '%' || $3 || '%' \
                 ORDER BY last_seen_at DESC",
            vec![filter, filter, filter],
            &paging_info
        )
    }
}
