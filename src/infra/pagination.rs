use crate::infra::db::SharedDatabase;
use crate::prelude::*;
use num_format::{Locale, ToFormattedString};
use sqlx::AssertSqlSafe;

#[derive(Deserialize)]
pub struct PagingInfo {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// The largest page a request may ask for.
pub const MAX_PAGE_SIZE: i64 = 100;

impl PagingInfo {
    /// `page` and `page_size` come straight from the query string, so they are
    /// pulled into range: page 1 or later, 1 to `MAX_PAGE_SIZE` rows, and no
    /// page so far out that the offset overflows.
    pub fn clamped(&self) -> (i64, i64) {
        let page_size = self.page_size.unwrap_or(15).clamp(1, MAX_PAGE_SIZE);
        let page = self.page.unwrap_or(1).clamp(1, i64::MAX / MAX_PAGE_SIZE);
        (page, page_size)
    }
}

/// The filter part of a clause, for the COUNT query, whatever case its
/// `ORDER BY` is written in.
fn without_order_by(clause: &str) -> &str {
    match clause.to_ascii_uppercase().find("ORDER BY") {
        Some(at) => &clause[..at],
        None => clause,
    }
}

impl Default for PagingInfo {
    fn default() -> Self {
        Self {
            page: Some(1),
            page_size: Some(15),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub start: i64,
    pub end: i64,
    pub page: i64,
    pub page_size: i64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

impl<T> Paginated<T> {
    /// Human-readable total with thousands separators (e.g. `"2,689"`).
    pub fn total_formatted(&self) -> String {
        self.total.to_formatted_string(&Locale::en)
    }

    /// Total number of pages, ceiling-divided. Returns at least 1 even when
    /// the total is 0 so "Page 1 of 1" still reads correctly on an empty set.
    pub fn total_pages(&self) -> i64 {
        if self.page_size <= 0 {
            1
        } else {
            std::cmp::max(1, (self.total + self.page_size - 1) / self.page_size)
        }
    }

    pub fn new(data: Vec<T>, total: i64, page: i64, page_size: i64) -> Self {
        let offset = (page - 1) * page_size;
        let has_previous_page = page > 1;
        let has_next_page = offset + (data.len() as i64) < total;
        let start = (page - 1) * page_size + 1;
        let end = std::cmp::min(page * page_size, total);

        Self {
            data,
            total,
            start,
            end,
            page,
            page_size,
            has_next_page,
            has_previous_page,
        }
    }
}

impl<T> Default for Paginated<T> {
    fn default() -> Self {
        Self {
            data: vec![],
            total: 0,
            page: 1,
            start: 1,
            end: 1,
            page_size: 15,
            has_next_page: false,
            has_previous_page: false,
        }
    }
}

impl<T> IntoIterator for Paginated<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

#[allow(async_fn_in_trait)]
pub trait Paginate:
    Sized + Send + Unpin + for<'r> FromRow<'r, sqlx::postgres::PgRow> + Serialize
{
    fn table_name() -> &'static str;

    fn count_query() -> String {
        format!("SELECT COUNT(*) FROM {}", Self::table_name())
    }

    fn page_query() -> String {
        format!("SELECT * FROM {}", Self::table_name())
    }

    async fn paginate(pool: &SharedDatabase, pagination: &PagingInfo) -> Result<Paginated<Self>> {
        Self::paginate_filter(pool, None, vec![], pagination).await
    }

    async fn paginate_filter(
        pool: &SharedDatabase,
        where_clause: Option<&str>,
        args: Vec<&str>,
        pagination: &PagingInfo,
    ) -> Result<Paginated<Self>> {
        let (page, page_size) = pagination.clamped();
        let offset = (page - 1) * page_size;

        let count_sql = if let Some(wc) = where_clause {
            format!("{} {}", Self::count_query(), without_order_by(wc))
        } else {
            Self::count_query().to_string()
        };

        let mut total_query = sqlx::query_as(AssertSqlSafe(count_sql));
        for arg in args.iter() {
            total_query = total_query.bind(arg);
        }
        let total: (i64,) = total_query.fetch_one(pool.as_ref()).await?;
        let arg_count = args.len();

        let page_sql = if let Some(wc) = where_clause {
            format!(
                "{} {} LIMIT ${} OFFSET ${}",
                Self::page_query(),
                wc,
                arg_count + 1,
                arg_count + 2
            )
        } else {
            format!(
                "{} ORDER BY id DESC LIMIT ${} OFFSET ${}",
                Self::page_query(),
                arg_count + 1,
                arg_count + 2
            )
        };
        let mut rows_query = sqlx::query_as::<_, Self>(AssertSqlSafe(page_sql));
        for arg in args.iter() {
            rows_query = rows_query.bind(arg);
        }
        let rows = rows_query
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool.as_ref())
            .await?;

        Ok(Paginated::new(rows, total.0, page, page_size))
    }
}

/// Paginate every row in the entity's table with default paging info.
///
/// ```ignore
/// paginate!(User, &db);
/// paginate!(User, &db, &paging);
/// ```
#[macro_export]
macro_rules! paginate {
    ($entity:ty, $db:expr) => {
        <$entity as $crate::infra::pagination::Paginate>::paginate(
            $db,
            &$crate::infra::pagination::PagingInfo::default(),
        )
        .await
    };
    ($entity:ty, $db:expr, $paging:expr) => {
        <$entity as $crate::infra::pagination::Paginate>::paginate($db, $paging).await
    };
}

/// Paginate with a `WHERE` clause and the parameters that fill it.
///
/// ```ignore
/// paginate_with!(User, &db, "where role = $1", vec!["admin"]);
/// paginate_with!(User, &db, "where role = $1", vec!["admin"], &paging);
/// ```
#[macro_export]
macro_rules! paginate_with {
    ($entity:ty, $db:expr, $where:expr, $args:expr) => {
        <$entity as $crate::infra::pagination::Paginate>::paginate_filter(
            $db,
            Some($where),
            $args,
            &$crate::infra::pagination::PagingInfo::default(),
        )
        .await
    };
    ($entity:ty, $db:expr, $where:expr, $args:expr, $paging:expr) => {
        <$entity as $crate::infra::pagination::Paginate>::paginate_filter(
            $db,
            Some($where),
            $args,
            $paging,
        )
        .await
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paging(page: i64, page_size: i64) -> PagingInfo {
        PagingInfo {
            page: Some(page),
            page_size: Some(page_size),
        }
    }

    #[test]
    fn out_of_range_paging_is_pulled_back_into_range() {
        assert_eq!(paging(0, 15).clamped(), (1, 15));
        assert_eq!(paging(-3, -5).clamped(), (1, 1));
        assert_eq!(paging(2, 1_000_000).clamped(), (2, MAX_PAGE_SIZE));
        let (page, size) = paging(i64::MAX, MAX_PAGE_SIZE).clamped();
        assert!(
            page.checked_mul(size).is_some(),
            "the offset can't overflow"
        );
    }

    #[test]
    fn the_count_query_drops_order_by_in_any_case() {
        assert_eq!(without_order_by("WHERE a = $1 ORDER BY b"), "WHERE a = $1 ");
        assert_eq!(without_order_by("where a = $1 order by b"), "where a = $1 ");
        assert_eq!(without_order_by("WHERE a = $1"), "WHERE a = $1");
    }
}
