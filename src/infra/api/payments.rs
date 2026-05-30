use crate::SharedState;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::current_user::CurrentUser;
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use crate::use_cases::payments::checkout_use_case::Plan;
use axum::Form;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, post};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/upgrade", get(upgrade_page))
        .route("/checkout", post(checkout))
        .route("/checkout/success", get(checkout_success))
        .route("/manage-subscriptions", post(manage_subscriptions))
        .route("/stripe", post(handle_stripe_event))
}

#[derive(Template, WebTemplate)]
#[template(path = "payments/upgrade.html")]
struct UpgradeTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "payments/subscription_success.html")]
struct SubscriptionSuccessTemplate {
    shared: SharedContext,
    features: Vec<(&'static str, &'static str)>,
}

async fn upgrade_page(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    if !user.is_free() {
        return Ok(redirect!("/", &headers).into_response());
    }
    let shared = SharedContext::new(&state)
        .with_user(Some(user))
        .with_canonical_path("/upgrade")
        .with_meta(PageMeta::new().title("Upgrade").robots("noindex,nofollow"));
    Ok(UpgradeTemplate { shared }.into_response())
}

#[derive(Deserialize)]
struct CheckoutForm {
    plan: Plan,
}

async fn checkout(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
    Form(form): Form<CheckoutForm>,
) -> Result<impl IntoResponse> {
    let customer_id = state
        .payments_use_cases
        .create_customer
        .execute(&user)
        .await?;

    let checkout_url = state
        .payments_use_cases
        .checkout
        .execute(customer_id, form.plan)
        .await?
        .url
        .unwrap_or("/".to_string());

    Ok(redirect!(&checkout_url, &headers))
}

async fn checkout_success(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    if user.is_free() {
        return Ok(redirect!("/upgrade", &headers).into_response());
    }
    let features = vec![
        ("Unlimited usage", "Use every feature without limits."),
        ("Priority support", "Get help faster when you need it."),
        ("Early access", "Try new features before everyone else."),
    ];
    let shared = SharedContext::new(&state)
        .with_user(Some(user))
        .with_canonical_path("/checkout/success")
        .with_meta(PageMeta::new().title("Welcome").robots("noindex,nofollow"));
    Ok(SubscriptionSuccessTemplate { shared, features }.into_response())
}

async fn manage_subscriptions(
    State(state): State<SharedState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let customer_id = state
        .payments_use_cases
        .create_customer
        .execute(&user)
        .await?;

    let portal_url = state
        .payments_use_cases
        .manage_subscription
        .execute(customer_id)
        .await?
        .url;

    Ok(redirect!(&portal_url, &headers))
}

async fn handle_stripe_event(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse> {
    state
        .payments_use_cases
        .handle_stripe_event
        .execute(&headers, &body)
        .await
}
