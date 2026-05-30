use crate::domain::user::{User, UserForm};
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::pagination::{Paginated, PagingInfo};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch};
use axum_extra::extract::Form;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/users", get(users))
        .route("/users/{id}", delete(destroy_user))
        .route("/users/{id}", patch(edit_user))
        .route("/users/{id}/edit", get(edit_user_modal))
}

#[derive(Template, WebTemplate)]
#[template(path = "users/users.html")]
struct ManageUsersTemplate {
    shared: SharedContext,
    users: Paginated<User>,
    search_query: String,
    pagination_base: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "users/_edit_user.html")]
struct EditUserModal {
    user: User,
}

#[derive(Deserialize, Default)]
struct UserSearchParams {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    page_size: Option<i64>,
}

async fn users(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Query(params): Query<UserSearchParams>,
) -> Result<impl IntoResponse> {
    let search_query = params.q.unwrap_or_default();
    let paging = PagingInfo {
        page: params.page.or(Some(1)),
        page_size: params.page_size.or(Some(15)),
    };
    let pagination_base = format!("/users?q={search_query}&");
    Ok(ManageUsersTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(user))
            .with_canonical_path("/users")
            .with_meta(
                PageMeta::new()
                    .title("Manage Users")
                    .robots("noindex,nofollow"),
            ),
        users: state
            .user_use_cases
            .search
            .execute(&search_query, &paging)
            .await?,
        search_query,
        pagination_base,
    })
}

async fn edit_user_modal(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
) -> Result<impl IntoResponse> {
    let user = User::find_by_id(state.db.as_ref(), id).await?;
    Ok(EditUserModal { user }.into_response())
}

async fn edit_user(
    State(state): State<SharedState>,
    Admin(_): Admin,
    Path(id): Path<PrimaryKey>,
    headers: HeaderMap,
    Form(form): Form<UserForm>,
) -> Result<impl IntoResponse> {
    User::update(state.db.as_ref(), id, &form).await?;
    Ok(redirect!("/users", &headers))
}

async fn destroy_user(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    User::destroy(state.db.as_ref(), id).await?;
    Ok(redirect!("/users", &headers))
}
