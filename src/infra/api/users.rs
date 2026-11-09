use crate::domain::account_group::AccountGroup;
use crate::domain::sign_in::{SIGN_IN_RETENTION_DAYS, SharedIpAccount, SignIn, SignInLocation};
use crate::domain::user::{Role, User, UserForm, UserUpdate};
use crate::domain::user_activity::{ACTIVITY_WINDOW_DAYS, ActiveDay};
use crate::domain::user_flag::UserFlag;
use crate::domain::user_note::{UserNote, UserNoteForm};
use crate::domain::value_objects::email::Email;
use crate::infra::api::SharedContext;
use crate::infra::api::auth::auth_cookie;
use crate::infra::api::extract::admin::Admin;
use crate::infra::api::extract::validated_form::ValidatedForm;
use crate::infra::pagination::{Paginated, PagingInfo};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use crate::use_cases::user::search_use_case::RoleFilter;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum_extra::extract::{CookieJar, Form};
use std::collections::{HashMap, HashSet};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/users", get(users))
        .route(
            "/users/{id}",
            get(user_page).patch(edit_user).delete(destroy_user),
        )
        .route("/users/{id}/edit", get(edit_user_modal))
        .route("/users/{id}/notes", post(create_note))
        .route("/users/{id}/notes/{note_id}", delete(destroy_note))
        .route("/users/{id}/sign-ins", get(sign_ins_card))
        .route("/users/{id}/flag", post(flag_user).delete(unflag_user))
        .route("/users/{id}/links", post(link_account))
        .route("/users/{id}/links/{other_id}", delete(unlink_account))
}

#[derive(Template, WebTemplate)]
#[template(path = "users/users.html")]
struct ManageUsersTemplate {
    shared: SharedContext,
    users: Paginated<User>,
    search_query: String,
    role_filter: RoleFilter,
    role_filters: [RoleFilter; 4],
    pagination_base: String,
    /// How many other accounts each listed user is linked to (linked only).
    linked_counts: HashMap<PrimaryKey, i64>,
    flagged_only: bool,
    /// Which listed users are flagged.
    flagged: HashSet<PrimaryKey>,
}

#[derive(Template, WebTemplate)]
#[template(path = "users/user.html")]
struct UserPageTemplate {
    shared: SharedContext,
    user: User,
    activity: Vec<ActiveDay>,
    activity_window_days: i32,
    notes: Vec<UserNote>,
    flag: Option<UserFlag>,
    sign_ins: Paginated<SignInLocation>,
    sign_ins_base: String,
    sign_in_retention_days: i32,
    linked: Vec<User>,
    suggestions: Vec<SharedIpAccount>,
    suggestion_total: i64,
    link_email: String,
    link_error: Option<String>,
}

impl UserPageTemplate {
    fn active_day_count(&self) -> usize {
        self.activity.iter().filter(|d| d.active).count()
    }
}

/// The "Sign-ins" card on its own, for paging through it over HTMX.
#[derive(Template, WebTemplate)]
#[template(path = "users/_sign_ins.html")]
struct SignInsPartial {
    sign_ins: Paginated<SignInLocation>,
    sign_ins_base: String,
    sign_in_retention_days: i32,
}

/// Rows per page on the Sign-ins card.
const SIGN_INS_PAGE_SIZE: i64 = 5;

#[derive(Deserialize)]
struct SignInsQuery {
    #[serde(default)]
    page: Option<i64>,
}

fn sign_ins_base(user_id: PrimaryKey) -> String {
    format!("/users/{user_id}/sign-ins?")
}

/// The "Linked accounts" card, swapped in place after a link or unlink.
#[derive(Template, WebTemplate)]
#[template(path = "users/_links.html")]
struct LinksPartial {
    user: User,
    linked: Vec<User>,
    suggestions: Vec<SharedIpAccount>,
    suggestion_total: i64,
    link_email: String,
    link_error: Option<String>,
}

/// How many same-IP accounts the user page lists before summarising.
const SUGGESTION_LIMIT: i64 = 10;

impl LinksPartial {
    async fn load(db: &sqlx::PgPool, user: User) -> Result<Self> {
        let (linked, suggestions) = tokio::join!(
            AccountGroup::linked_accounts(db, user.id),
            SignIn::accounts_sharing_ips(db, user.id, SUGGESTION_LIMIT),
        );
        let (suggestions, suggestion_total) = suggestions?;
        Ok(Self {
            user,
            linked: linked?,
            suggestions,
            suggestion_total,
            link_email: String::new(),
            link_error: None,
        })
    }
}

#[derive(Deserialize)]
struct LinkAccountForm {
    email: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "users/_notes.html")]
struct NotesPartial {
    user: User,
    notes: Vec<UserNote>,
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
    role: Option<String>,
    /// Any non-empty value (the checkbox sends `on`) means "flagged only".
    #[serde(default)]
    flagged: Option<String>,
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
    let role_filter = RoleFilter::parse(params.role.as_deref());
    let flagged_only = params.flagged.is_some_and(|v| !v.is_empty());
    let paging = PagingInfo {
        page: params.page.or(Some(1)),
        page_size: params.page_size.or(Some(15)),
    };
    let pagination_base = format!(
        "/users?q={}&role={}&{}",
        encode_query_value(&search_query),
        role_filter.as_str(),
        if flagged_only { "flagged=on&" } else { "" }
    );
    let users = state
        .user_use_cases
        .search
        .execute(&search_query, role_filter, flagged_only, &paging)
        .await?;
    let ids: Vec<PrimaryKey> = users.data.iter().map(|u| u.id).collect();
    let (linked_counts, flagged) = tokio::join!(
        AccountGroup::linked_counts(state.db.as_ref(), &ids),
        UserFlag::flagged_among(state.db.as_ref(), &ids),
    );
    Ok(ManageUsersTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(user))
            .with_canonical_path("/users")
            .with_meta(
                PageMeta::new()
                    .title("Manage Users")
                    .robots("noindex,nofollow"),
            ),
        users,
        search_query,
        role_filter,
        role_filters: RoleFilter::ALL,
        pagination_base,
        linked_counts: linked_counts?,
        flagged_only,
        flagged: flagged?,
    })
}

/// Percent-encodes a query-string value so a search like `a&b` survives the
/// round trip through the paginator links.
fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The admin's one-stop view of an account: account facts, activity, sign-in
/// history, linked accounts, and private admin notes. Editing still happens
/// in the modal (`edit_user_modal`), opened from this page.
async fn user_page(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_read(&user)?;
    let (activity, notes, sign_ins, linked, suggestions, flag) = tokio::join!(
        ActiveDay::recent_for_user(db, user.id),
        UserNote::find_by_user_id(db, user.id),
        SignIn::locations_for_user(db, user.id, 1, SIGN_INS_PAGE_SIZE),
        AccountGroup::linked_accounts(db, user.id),
        SignIn::accounts_sharing_ips(db, user.id, SUGGESTION_LIMIT),
        UserFlag::find(db, user.id),
    );
    let (suggestions, suggestion_total) = suggestions?;
    let page_title = user.full_name.clone();
    Ok(UserPageTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(admin))
            .with_canonical_path(format!("/users/{}", user.id))
            .with_meta(PageMeta::new().title(page_title).robots("noindex,nofollow")),
        sign_ins_base: sign_ins_base(user.id),
        user,
        activity: activity?,
        activity_window_days: ACTIVITY_WINDOW_DAYS,
        notes: notes?,
        flag: flag?,
        sign_ins: sign_ins?,
        sign_in_retention_days: SIGN_IN_RETENTION_DAYS,
        linked: linked?,
        suggestions,
        suggestion_total,
        link_email: String::new(),
        link_error: None,
    })
}

/// Flags an account to keep an eye on it. Purely informational — the account
/// keeps working — and only admins ever see it.
async fn flag_user(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let user = User::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&user)?;
    UserFlag::flag(state.db.as_ref(), user.id, admin.id).await?;
    Ok(refresh!(headers))
}

async fn unflag_user(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let user = User::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&user)?;
    UserFlag::unflag(state.db.as_ref(), user.id).await?;
    Ok(refresh!(headers))
}

async fn sign_ins_card(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Query(query): Query<SignInsQuery>,
    Admin(admin): Admin,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_read(&user)?;
    let page = query.page.unwrap_or(1);
    Ok(SignInsPartial {
        sign_ins: SignIn::locations_for_user(db, user.id, page, SIGN_INS_PAGE_SIZE).await?,
        sign_ins_base: sign_ins_base(user.id),
        sign_in_retention_days: SIGN_IN_RETENTION_DAYS,
    })
}

async fn link_account(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    Form(form): Form<LinkAccountForm>,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_write(&user)?;
    let result = match Email::parse(&form.email) {
        Ok(email) => state
            .user_use_cases
            .link_accounts
            .execute(&admin, &user, &email)
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    let mut partial = LinksPartial::load(db, user).await?;
    match result {
        Ok(()) => {}
        // Shown inline, next to the field the admin just typed into.
        Err(AppError::NotFound(message)) | Err(AppError::BadRequest(message)) => {
            partial.link_email = form.email.trim().to_string();
            partial.link_error = Some(message);
        }
        Err(error) => return Err(error),
    }
    Ok(partial)
}

async fn unlink_account(
    State(state): State<SharedState>,
    Path((id, other_id)): Path<(PrimaryKey, PrimaryKey)>,
    Admin(admin): Admin,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_write(&user)?;
    state
        .user_use_cases
        .unlink_account
        .execute(&admin, &user, other_id)
        .await?;
    LinksPartial::load(db, user).await
}

async fn create_note(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    ValidatedForm(form): ValidatedForm<UserNoteForm>,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_write(&user)?;
    UserNote::create(db, user.id, admin.id, &form).await?;
    let notes = UserNote::find_by_user_id(db, user.id).await?;
    Ok(NotesPartial { user, notes })
}

async fn destroy_note(
    State(state): State<SharedState>,
    Path((id, note_id)): Path<(PrimaryKey, PrimaryKey)>,
    Admin(admin): Admin,
) -> Result<impl IntoResponse> {
    let db = state.db.as_ref();
    let user = User::find_by_id(db, id).await?;
    admin.ensure_can_write(&user)?;
    UserNote::destroy(db, user.id, note_id).await?;
    let notes = UserNote::find_by_user_id(db, user.id).await?;
    Ok(NotesPartial { user, notes })
}

async fn edit_user_modal(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
) -> Result<impl IntoResponse> {
    let user = User::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&user)?;
    Ok(EditUserModal { user })
}

/// Editing the email is how an admin swaps the login behind an existing
/// account: everything hangs off `users.id`, so the Stripe customer and the
/// rest of the account stay put while the address that signs in changes. The
/// old address stops working immediately — its JWT no longer resolves to a
/// row — and the new one picks the account up on its next sign-in.
async fn edit_user(
    State(state): State<SharedState>,
    Admin(admin): Admin,
    Path(id): Path<PrimaryKey>,
    headers: HeaderMap,
    cookies: CookieJar,
    Form(form): Form<UserForm>,
) -> Result<impl IntoResponse> {
    let update = UserUpdate::try_from(form)?;
    let existing = User::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&existing)?;
    let updated = User::update(state.db.as_ref(), existing.id, &update).await?;

    // An admin editing their own row would otherwise be holding a cookie keyed
    // to the address they just replaced, and the redirect below would bounce
    // them out. Hand them a token for the new address instead.
    let cookies = if admin.id == updated.id && updated.email != existing.email {
        cookies.add(auth_cookie(&updated)?)
    } else {
        cookies
    };

    Ok((cookies, redirect!(&format!("/users/{id}"), &headers)))
}

async fn destroy_user(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let user = User::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&user)?;

    // Admins can never be deleted through this surface — not by other admins
    // and not by themselves. Demote the role first if removal is intended.
    if user.is_admin() {
        return Err(AppError::Forbidden("Admins cannot be deleted.".into()));
    }

    if let Some(customer_id) = &user.stripe_customer_id {
        state
            .payments_use_cases
            .delete_customer
            .execute(customer_id)
            .await?;
    }

    User::destroy(state.db.as_ref(), user.id).await?;
    Ok(redirect!("/users", &headers))
}

#[cfg(test)]
mod tests {
    use super::encode_query_value;

    #[test]
    fn encodes_reserved_characters() {
        assert_eq!(encode_query_value("a&b c"), "a%26b%20c");
        assert_eq!(encode_query_value("ada@example.com"), "ada%40example.com");
    }
}
