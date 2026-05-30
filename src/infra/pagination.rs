use crate::infra::db::SharedDatabase;
use crate::prelude::*;
use num_format::{Locale, ToFormattedString};
use sqlx::AssertSqlSafe;

#[derive(Deserialize)]
pub struct PagingInfo {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
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
        let page = pagination.page.unwrap_or(1);
        let page_size = pagination.page_size.unwrap_or(15);
        let offset = (page - 1) * page_size;

        let count_sql = if let Some(wc) = where_clause {
            let filter_only = wc
                .split_inclusive("ORDER BY")
                .next()
                .and_then(|s| s.strip_suffix("ORDER BY"))
                .unwrap_or(wc);
            format!("{} {}", Self::count_query(), filter_only)
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
