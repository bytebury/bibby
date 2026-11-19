use crate::domain::user::Role;
use crate::infra::payments::Stripe;
use crate::prelude::*;
use shima::checkout::{CheckoutSession, CreateCheckoutSession};
use shima::{CustomerId, PriceId};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    MonthlyPro,
    AnnualPro,
}

impl Plan {
    fn env_var(&self) -> &'static str {
        match self {
            Plan::MonthlyPro => "STRIPE_MONTHLY_PRO",
            Plan::AnnualPro => "STRIPE_ANNUAL_PRO",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Plan::MonthlyPro => "monthly_pro",
            Plan::AnnualPro => "annual_pro",
        }
    }

    pub fn role(&self) -> Role {
        match self {
            Plan::MonthlyPro | Plan::AnnualPro => Role::Professional,
        }
    }
}

impl std::str::FromStr for Plan {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "monthly_pro" => Ok(Plan::MonthlyPro),
            "annual_pro" => Ok(Plan::AnnualPro),
            other => Err(AppError::BadRequest(format!("Unknown plan {other:?}."))),
        }
    }
}

pub struct CheckoutUseCase {
    stripe: Stripe,
}

impl CheckoutUseCase {
    pub fn new(stripe: &Stripe) -> Self {
        Self {
            stripe: stripe.clone(),
        }
    }

    pub async fn execute(&self, customer_id: CustomerId, plan: Plan) -> Result<CheckoutSession> {
        let website_url = app_origin()?;
        let success_url = format!("{}/checkout/success", website_url);
        let price = env::var(plan.env_var()).map_err(|_| {
            AppError::BadRequest(format!("Plan {} is not configured.", plan.env_var()))
        })?;
        let price_id = PriceId::try_from(price)
            .map_err(|e| AppError::Internal(format!("Invalid Stripe price id: {e}")))?;

        let mut session = CreateCheckoutSession::new_subscription(
            customer_id,
            price_id,
            success_url.into(),
            website_url.into(),
        );
        session.client_reference_id = Some(plan.as_str().to_string());

        CheckoutSession::create(&self.stripe.client, session)
            .await
            .map_err(Into::into)
    }
}

/// Where Stripe sends people back to: this app's public origin.
pub fn app_origin() -> Result<String> {
    env::var("APP_ORIGIN")
        .ok()
        .map(|origin| origin.trim_end_matches('/').to_string())
        .filter(|origin| !origin.is_empty())
        .ok_or_else(|| AppError::Internal("APP_ORIGIN must be set to use Stripe".to_string()))
}
