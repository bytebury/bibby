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
    Unknown,
}

impl Plan {
    fn env_var(&self) -> &'static str {
        match self {
            Plan::MonthlyPro => "STRIPE_MONTHLY_PRO",
            Plan::AnnualPro => "STRIPE_ANNUAL_PRO",
            Plan::Unknown => "UNKNOWN",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Plan::MonthlyPro => "monthly_pro",
            Plan::AnnualPro => "annual_pro",
            Plan::Unknown => "unknown",
        }
    }

    pub fn role(&self) -> Role {
        match self {
            Plan::MonthlyPro | Plan::AnnualPro => Role::Professional,
            _ => Role::Professional,
        }
    }
}

impl std::str::FromStr for Plan {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "monthly_pro" => Ok(Plan::MonthlyPro),
            "annual_pro" => Ok(Plan::AnnualPro),
            _ => Ok(Plan::MonthlyPro),
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
        let website_url = env::var("WEBSITE_URL").expect("WEBSITE_URL must be set");
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
