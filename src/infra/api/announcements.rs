use crate::domain::announcement::{Announcement, AnnouncementForm};
use crate::domain::value_objects::toggle::Toggle;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::api::extract::validated_form::ValidatedForm;
use crate::infra::pagination::{Paginate, Paginated, PagingInfo};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, put};
use axum_extra::extract::Form;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/announcements", get(announcements))
        .route("/announcements", put(create_announcement))
        .route("/announcements/banner", get(banner))
        .route("/announcements/new", get(new_announcement_modal))
        .route("/announcements/{id}", delete(destroy_announcement))
        .route("/announcements/{id}", patch(edit_announcement))
        .route("/announcements/{id}/active", patch(toggle_active))
        .route("/announcements/{id}/edit", get(edit_announcement_modal))
}

#[derive(Deserialize, Default)]
struct ActiveForm {
    #[serde(default)]
    active: Toggle,
}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/announcements.html")]
struct AnnouncementsTemplate {
    shared: SharedContext,
    announcements: Paginated<Announcement>,
}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/_new_announcement.html")]
struct NewAnnouncementModal {}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/_edit_announcement.html")]
struct EditAnnouncementModal {
    announcement: Announcement,
}

#[derive(Template, WebTemplate)]
#[template(path = "_partials/announcement_banner.html")]
struct BannerTemplate {
    announcement: Option<Announcement>,
}

async fn announcements(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Query(params): Query<PagingInfo>,
) -> Result<AnnouncementsTemplate> {
    Ok(AnnouncementsTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(user))
            .with_canonical_path("/announcements")
            .with_meta(
                PageMeta::new()
                    .title("Announcements")
                    .robots("noindex,nofollow"),
            ),
        announcements: Announcement::paginate(&state.db, &params).await?,
    })
}

async fn new_announcement_modal(Admin(_): Admin) -> NewAnnouncementModal {
    NewAnnouncementModal {}
}

async fn create_announcement(
    State(state): State<SharedState>,
    Admin(user): Admin,
    headers: HeaderMap,
    ValidatedForm(form): ValidatedForm<AnnouncementForm>,
) -> Result<impl IntoResponse> {
    Announcement::create(state.db.as_ref(), user.id, &form).await?;
    Ok(redirect!("/announcements", &headers))
}

async fn edit_announcement_modal(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
) -> Result<impl IntoResponse> {
    let announcement = Announcement::find_by_id(state.db.as_ref(), id).await?;
    Ok(EditAnnouncementModal { announcement }.into_response())
}

async fn edit_announcement(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Path(id): Path<PrimaryKey>,
    headers: HeaderMap,
    ValidatedForm(form): ValidatedForm<AnnouncementForm>,
) -> Result<impl IntoResponse> {
    Announcement::update(state.db.as_ref(), id, user.id, &form).await?;
    Ok(redirect!("/announcements", &headers))
}

async fn destroy_announcement(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    Announcement::destroy(state.db.as_ref(), id).await?;
    Ok(redirect!("/announcements", &headers))
}

async fn toggle_active(
    State(state): State<SharedState>,
    Admin(_): Admin,
    Path(id): Path<PrimaryKey>,
    Form(form): Form<ActiveForm>,
) -> Result<impl IntoResponse> {
    Announcement::set_active(state.db.as_ref(), id, form.active.as_bool()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Public endpoint — every page fetches this on load via HTMX so the latest
/// active announcement renders sitewide. Returns an empty `<div>` when there
/// is no active announcement so the placeholder cleanly swaps itself out.
async fn banner(State(state): State<SharedState>) -> Result<BannerTemplate> {
    Ok(BannerTemplate {
        announcement: Announcement::find_latest_active(state.db.as_ref())
            .await
            .unwrap_or(None),
    })
}
