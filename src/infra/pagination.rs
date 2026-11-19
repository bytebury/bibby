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

#[allow(async_fn_in_trait)]
pub trait Paginate:
    Sized + Send + Unpin + for<'r> FromRow<'r, sqlx::postgres::PgRow> + Serialize
{
    fn table_name() -> &'static str;

    /// A page of every row, newest (highest id) first.
    async fn paginate(db: &SharedDatabase, paging: &PagingInfo) -> Result<Paginated<Self>> {
        Self::paginate_where(db, "ORDER BY id DESC", vec![], paging).await
    }

    /// A page of the rows `clause` selects: an optional `WHERE … ` followed
    /// by its `ORDER BY`. `args` fill its `$1`, `$2`, … in order. Only fixed
    /// SQL goes in `clause`, never request data.
    async fn paginate_where(
        db: &SharedDatabase,
        clause: &str,
        args: Vec<&str>,
        paging: &PagingInfo,
    ) -> Result<Paginated<Self>> {
        let (page, page_size) = paging.clamped();
        let table = Self::table_name();

        let count_sql = format!("SELECT COUNT(*) FROM {table} {}", without_order_by(clause));
        let mut count = sqlx::query_as(AssertSqlSafe(count_sql));
        for arg in &args {
            count = count.bind(arg);
        }

        let page_sql = format!(
            "SELECT * FROM {table} {clause} LIMIT ${} OFFSET ${}",
            args.len() + 1,
            args.len() + 2
        );
        let mut rows = sqlx::query_as::<_, Self>(AssertSqlSafe(page_sql));
        for arg in &args {
            rows = rows.bind(arg);
        }
        let rows = rows.bind(page_size).bind((page - 1) * page_size);

        let ((total,), rows) =
            tokio::try_join!(count.fetch_one(db.as_ref()), rows.fetch_all(db.as_ref()))?;
        Ok(Paginated::new(rows, total, page, page_size))
    }
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
