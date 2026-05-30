use crate::SharedState;
use crate::domain::country::Country;
use crate::domain::value_objects::toggle::Toggle;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, patch};
use axum_extra::extract::Form;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/countries", get(countries))
        .route("/countries/{id}/lock", patch(toggle_lock))
}

#[derive(Template, WebTemplate)]
#[template(path = "countries/countries.html")]
struct CountriesTemplate {
    shared: SharedContext,
    countries: Vec<Country>,
    search_query: String,
}

#[derive(Deserialize, Default)]
struct CountrySearchParams {
    #[serde(default)]
    q: Option<String>,
}

#[derive(Deserialize, Default)]
struct LockForm {
    #[serde(default)]
    locked: Toggle,
}

async fn countries(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Query(params): Query<CountrySearchParams>,
) -> Result<CountriesTemplate> {
    let search_query = params.q.unwrap_or_default();
    let countries = Country::search(state.db.as_ref(), &search_query).await?;
    Ok(CountriesTemplate {
        shared: SharedContext::new(&state).with_user(Some(user)),
        countries,
        search_query,
    })
}

async fn toggle_lock(
    State(state): State<SharedState>,
    Admin(_): Admin,
    Path(id): Path<PrimaryKey>,
    Form(form): Form<LockForm>,
) -> Result<impl IntoResponse> {
    Country::set_locked(state.db.as_ref(), id, form.locked.as_bool()).await?;
    Ok(StatusCode::NO_CONTENT)
}
