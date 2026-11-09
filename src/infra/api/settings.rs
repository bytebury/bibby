use crate::domain::user::{NameForm, NameUpdate, User};
use crate::infra::api::SharedContext;
use crate::infra::api::extract::current_user::CurrentUser;
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Form;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;

// Settings only ever touch the caller's own row: the user comes from the
// session, never from an id in the request, so there is no entity to check
// access against.
pub fn routes() -> Router<SharedState> {
    Router::new().route("/settings", get(settings_page).patch(update_name))
}

#[derive(Template, WebTemplate)]
#[template(path = "settings/settings.html")]
struct SettingsTemplate {
    shared: SharedContext,
    user: User,
}

async fn settings_page(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
) -> Result<impl IntoResponse> {
    let shared = SharedContext::new(&state)
        .with_user(Some(user.clone()))
        .with_canonical_path("/settings")
        .with_meta(PageMeta::new().title("Settings").robots("noindex,nofollow"));
    Ok(SettingsTemplate { shared, user })
}

async fn update_name(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
    Form(form): Form<NameForm>,
) -> Result<impl IntoResponse> {
    let update = NameUpdate::try_from(form)?;
    User::update_name(state.db.as_ref(), user.id, &update).await?;
    Ok(refresh!(&headers))
}
