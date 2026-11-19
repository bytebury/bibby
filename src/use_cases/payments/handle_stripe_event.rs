use crate::domain::user::{Role, User};
use crate::infra::db::SharedDatabase;
use crate::infra::payments::Stripe;
use crate::prelude::*;
use crate::use_cases::payments::checkout_use_case::Plan;
use axum::http::HeaderMap;
use shima::CustomerId;
use shima::checkout::CheckoutSession;
use shima::subscription::SubscriptionStatus;
use shima::webhook::event::ShimaEvent;
use std::str::FromStr;

pub struct HandleStripeEventUseCase {
    stripe: Stripe,
    db: SharedDatabase,
}

impl HandleStripeEventUseCase {
    pub fn new(stripe: &Stripe, db: SharedDatabase) -> Self {
        Self {
            stripe: stripe.clone(),
            db,
        }
    }

    /// Applies a Stripe event to the account it belongs to.
    ///
    /// The subscription's status decides the plan: active or trialing keeps
    /// it, and canceled, unpaid, paused or expired takes it away. A failed
    /// invoice changes nothing by itself, since Stripe keeps retrying the card
    /// and only marks the subscription unpaid once it gives up. Admins keep
    /// their role whatever Stripe says, and events for customers this app
    /// doesn't know (e.g. one deleted along with its user) are acknowledged
    /// so Stripe stops retrying them.
    pub async fn execute(&self, headers: &HeaderMap, body: &str) -> Result<()> {
        let (customer, role) = match self.stripe.listener.process(headers, body)? {
            // Delayed payment methods complete the session unpaid and settle
            // later via `async_payment_succeeded`, so only grant access once
            // fulfilled.
            ShimaEvent::CheckoutSessionCompleted(session)
            | ShimaEvent::CheckoutSessionAsyncPaymentSucceeded(session)
                if session.is_fulfilled() =>
            {
                fulfilled_plan(&session)?
            }
            ShimaEvent::CustomerSubscriptionUpdated(subscription) => {
                match plan_change(subscription.status) {
                    // A subscription that recovered (e.g. the card was fixed
                    // after a failed payment) gets its plan back.
                    PlanChange::Grant => (subscription.customer, Role::Professional),
                    PlanChange::Revoke => (subscription.customer, Role::Free),
                    PlanChange::Keep => return Ok(()),
                }
            }
            ShimaEvent::CustomerSubscriptionDeleted(subscription) => {
                (subscription.customer, Role::Free)
            }
            _ => return Ok(()),
        };

        let user = match User::find_by_stripe_customer_id(self.db.as_ref(), &customer).await {
            Ok(user) => user,
            Err(AppError::NotFound(_)) => {
                warn!("ignoring a Stripe event for a customer with no account");
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        // Admins keep their role whatever Stripe says. An account already on
        // `role` with no admin-granted end date has nothing to change.
        if user.is_admin() || (user.role == role && user.plan_expiry_date.is_none()) {
            return Ok(());
        }

        let mut tx = self.db.begin().await?;
        User::set_role(&mut *tx, user.id, &role).await?;
        // Stripe now owns this plan's lifecycle, so any admin-granted expiry
        // date must go — otherwise the scheduler would downgrade a paying
        // customer on the old deadline.
        User::clear_plan_expiry(&mut *tx, user.id).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// The customer and plan a fulfilled checkout grants.
fn fulfilled_plan(session: &CheckoutSession) -> Result<(CustomerId, Role)> {
    let customer = session.customer.clone().ok_or_else(|| {
        AppError::BadRequest("checkout.session.completed missing customer".into())
    })?;
    let plan = session.client_reference_id.as_deref().ok_or_else(|| {
        AppError::BadRequest("checkout.session.completed missing client_reference_id".into())
    })?;
    Ok((customer, Plan::from_str(plan)?.role()))
}

#[derive(Debug, PartialEq, Eq)]
enum PlanChange {
    Grant,
    Revoke,
    Keep,
}

/// What a subscription in `status` means for the customer's plan. `past_due`
/// keeps it while Stripe retries the payment, and `incomplete` is a first
/// payment still in progress; checkout grants the plan once it settles.
fn plan_change(status: SubscriptionStatus) -> PlanChange {
    match status {
        SubscriptionStatus::Active | SubscriptionStatus::Trialing => PlanChange::Grant,
        SubscriptionStatus::Canceled
        | SubscriptionStatus::Unpaid
        | SubscriptionStatus::Paused
        | SubscriptionStatus::IncompleteExpired => PlanChange::Revoke,
        SubscriptionStatus::PastDue
        | SubscriptionStatus::Incomplete
        | SubscriptionStatus::Unknown => PlanChange::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retrying_payment_keeps_the_plan() {
        assert_eq!(plan_change(SubscriptionStatus::PastDue), PlanChange::Keep);
    }

    #[test]
    fn a_recovered_subscription_gets_the_plan_back() {
        assert_eq!(plan_change(SubscriptionStatus::Active), PlanChange::Grant);
        assert_eq!(plan_change(SubscriptionStatus::Trialing), PlanChange::Grant);
    }

    #[test]
    fn an_unpaid_or_ended_subscription_loses_the_plan() {
        for status in [
            SubscriptionStatus::Unpaid,
            SubscriptionStatus::Canceled,
            SubscriptionStatus::Paused,
            SubscriptionStatus::IncompleteExpired,
        ] {
            assert_eq!(plan_change(status), PlanChange::Revoke, "{status}");
        }
    }
}
