use crate::domain::feature_toggle::{Feature, FeatureToggle};
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, patch};
use axum_extra::extract::Form;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/admin/feature-toggles", get(feature_toggles))
        .route("/admin/feature-toggles/{id}", patch(update_toggle))
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/feature_toggles.html")]
struct FeatureTogglesTemplate {
    shared: SharedContext,
    toggles: Vec<(FeatureToggle, Feature)>,
}

async fn feature_toggles(
    State(state): State<SharedState>,
    Admin(admin): Admin,
) -> Result<FeatureTogglesTemplate> {
    let toggles = FeatureToggle::all(state.db.as_ref())
        .await?
        .into_iter()
        .filter_map(|toggle| toggle.feature().map(|feature| (toggle, feature)))
        .collect();
    Ok(FeatureTogglesTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(admin))
            .with_canonical_path("/admin/feature-toggles")
            .with_meta(PageMeta::private("Feature Toggles")),
        toggles,
    })
}

#[derive(Deserialize)]
struct ToggleForm {
    /// A checkbox: browsers only send it when it is checked.
    enabled: Option<String>,
    #[serde(default)]
    value: String,
}

async fn update_toggle(
    State(state): State<SharedState>,
    Admin(admin): Admin,
    Path(id): Path<PrimaryKey>,
    headers: HeaderMap,
    Form(form): Form<ToggleForm>,
) -> Result<impl IntoResponse> {
    let toggle = FeatureToggle::find_by_id(state.db.as_ref(), id).await?;
    admin.ensure_can_write(&toggle)?;
    let feature = toggle
        .feature()
        .ok_or_else(|| AppError::NotFound("This feature no longer exists.".to_string()))?;
    let enabled = form.enabled.is_some();
    let value = feature.parse_value(enabled, &form.value)?;
    FeatureToggle::update(state.db.as_ref(), toggle.id, enabled, value.as_deref()).await?;
    state.site.reload().await;
    info!(feature = feature.key(), enabled, "feature toggle changed");
    Ok(refresh!(&headers))
}
