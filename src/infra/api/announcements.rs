use crate::domain::announcement::{Announcement, AnnouncementForm};
use crate::domain::value_objects::severity::Severity;
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
        .route("/announcements/new", get(new_announcement_modal))
        .route("/announcements/{id}", delete(destroy_announcement))
        .route("/announcements/{id}", patch(edit_announcement))
        .route("/announcements/{id}/active", patch(toggle_active))
        .route("/announcements/{id}/edit", get(edit_announcement_modal))
}

#[derive(Deserialize, Default)]
struct ActiveForm {
    #[serde(default)]
    /// A checkbox: browsers only send it when it is checked.
    active: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/announcements.html")]
struct AnnouncementsTemplate {
    shared: SharedContext,
    announcements: Paginated<Announcement>,
}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/_new_announcement.html")]
struct NewAnnouncementModal {
    /// The level the form starts on.
    severity: Severity,
}

#[derive(Template, WebTemplate)]
#[template(path = "announcements/_edit_announcement.html")]
struct EditAnnouncementModal {
    announcement: Announcement,
}

async fn announcements(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Query(params): Query<PagingInfo>,
) -> Result<AnnouncementsTemplate> {
    Ok(AnnouncementsTemplate {
        shared: SharedContext::new(&state)
            .await
            .with_user(Some(user))
            .with_canonical_path("/announcements")
            .with_meta(PageMeta::private("Announcements")),
        announcements: Announcement::paginate(&state.db, &params).await?,
    })
}

async fn new_announcement_modal(Admin(_): Admin) -> NewAnnouncementModal {
    NewAnnouncementModal {
        severity: Severity::default(),
    }
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
    let announcement = Announcement::find_by_id(state.db.as_ref(), id).await?;
    user.ensure_can_write(&announcement)?;
    Announcement::update(state.db.as_ref(), announcement.id, user.id, &form).await?;
    Ok(redirect!("/announcements", &headers))
}

async fn destroy_announcement(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(admin): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let announcement = Announcement::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&announcement)?;
    Announcement::destroy(state.db.as_ref(), announcement.id).await?;
    Ok(redirect!("/announcements", &headers))
}

async fn toggle_active(
    State(state): State<SharedState>,
    Admin(admin): Admin,
    Path(id): Path<PrimaryKey>,
    Form(form): Form<ActiveForm>,
) -> Result<impl IntoResponse> {
    let announcement = Announcement::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&announcement)?;
    Announcement::set_active(state.db.as_ref(), announcement.id, form.active.is_some()).await?;
    Ok(StatusCode::NO_CONTENT)
}
