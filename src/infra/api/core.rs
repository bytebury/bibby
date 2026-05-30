use crate::infra::api::SharedContext;
use crate::infra::api::extract::maybe_current_user::MaybeCurrentUser;
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::routing::get;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(homepage))
        .route("/privacy", get(privacy))
        .route("/terms", get(terms))
}

#[derive(Template, WebTemplate)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "privacy.html")]
struct PrivacyTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "terms.html")]
struct TermsTemplate {
    shared: SharedContext,
}

async fn homepage(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> HomepageTemplate {
    HomepageTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/"),
    }
}

async fn privacy(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> PrivacyTemplate {
    PrivacyTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/privacy"),
    }
}

async fn terms(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> TermsTemplate {
    TermsTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/terms"),
    }
}
