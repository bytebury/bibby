use crate::domain::city::City;
use crate::domain::country::Country;
use crate::domain::region::Region;
use crate::domain::sign_in::SignIn;
use crate::domain::user::{AuthProvider, CreateUser, User};
use crate::infra::db::SharedDatabase;
use crate::infra::geolocation::LocationDetails;
use crate::prelude::*;
use std::net::IpAddr;

pub struct RegisterUserUseCase {
    db: SharedDatabase,
}

impl RegisterUserUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    /// Resolve the user's IP to a location, then either create the user or
    /// refresh their audit fields, and record the sign-in for the admin user
    /// page. The automatic lock signals (see `resolve_location`) propagate as
    /// a sign-in block.
    pub async fn execute(&self, request: &mut CreateUser) -> Result<User> {
        self.resolve_location(request).await?;

        let user = match User::find_by_email(self.db.as_ref(), &request.email).await {
            Ok(user) => self.update_audit_fields(user, request).await,
            Err(_) => User::create(self.db.as_ref(), request).await,
        }?;

        // Recorded even for a locked account — those are the sign-ins an
        // admin most wants to see.
        SignIn::record(
            self.db.as_ref(),
            user.id,
            &request.last_known_ip,
            request.country_id,
            request.region_id,
            request.city_id,
        )
        .await?;

        Ok(user)
    }

    /// Fills in the country/region/city for `request.last_known_ip` (via
    /// `geodude` if configured), persisting any newly-seen rows, and sets
    /// `request.locked` from the two automatic lock signals: this sign-in came
    /// from a locked country, or from an IP a locked account was last seen on.
    pub async fn resolve_location(&self, request: &mut CreateUser) -> Result<()> {
        let location = match self.parse_ip(&request.last_known_ip) {
            Some(ip) => LocationDetails::lookup(ip).await.unwrap_or_default(),
            None => LocationDetails::default(),
        };

        let country = match Country::find_by_code(self.db.as_ref(), &location.country.code).await {
            Ok(country) => country,
            Err(_) => Country::create(self.db.as_ref(), &location.country.clone().into()).await?,
        };

        request.country_id = Some(country.id);

        if let Some(region_name) = location.region.as_deref() {
            let region = Region::find_or_create(self.db.as_ref(), country.id, region_name).await?;
            request.region_id = Some(region.id);

            if let Some(city_name) = location.city.as_deref() {
                let city = City::find_or_create(self.db.as_ref(), region.id, city_name).await?;
                request.city_id = Some(city.id);
            }
        }

        request.locked = country.locked
            || (!request.last_known_ip.is_empty()
                && User::any_locked_with_ip(self.db.as_ref(), &request.last_known_ip).await?);

        Ok(())
    }

    /// Whether `ip` trips either automatic lock signal: it resolves to a
    /// locked country, or a locked account was last seen on it. The same two
    /// signals `resolve_location` uses, but this writes nothing, so a blocked
    /// visitor leaves no trace in the database.
    pub async fn blocks_sign_up(&self, ip: &str) -> Result<bool> {
        if let Some(addr) = self.parse_ip(ip) {
            let location = LocationDetails::lookup(addr).await.unwrap_or_default();
            match Country::find_by_code(self.db.as_ref(), &location.country.code).await {
                Ok(country) if country.locked => return Ok(true),
                Ok(_) | Err(AppError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(!ip.is_empty() && User::any_locked_with_ip(self.db.as_ref(), ip).await?)
    }

    /// Whether a sign-in with `email` from `ip` must not create an account:
    /// the address has none yet and `blocks_sign_up` fires for the visitor.
    /// Existing accounts are untouched; they are locked by `execute` instead.
    pub async fn blocks_new_account(&self, email: &str, ip: &str) -> Result<bool> {
        match User::find_by_email(self.db.as_ref(), email).await {
            Ok(_) => Ok(false),
            Err(AppError::NotFound(_)) => self.blocks_sign_up(ip).await,
            Err(error) => Err(error),
        }
    }

    fn parse_ip(&self, ip_address: &str) -> Option<IpAddr> {
        ip_address.parse::<IpAddr>().ok()
    }

    async fn update_audit_fields(&self, user: User, request: &CreateUser) -> Result<User> {
        let user_id = user.id;

        // Google has just proven who owns the address, so an email sign-up
        // (verified or not) becomes a Google account and an unverified one is
        // verified. A code sign-in echoes the stored provider and skips this.
        if request.verified
            && request.auth_provider == AuthProvider::Google
            && (user.signs_in_with_email() || !user.verified)
        {
            User::convert_to_google(self.db.as_ref(), user_id, request).await?;
        }

        let locked = locked_after_sign_in(&user, request.locked);
        User::update_sign_in_fields(self.db.as_ref(), user_id, request, locked).await
    }
}

/// The lock an existing account carries away from this sign-in, given whether
/// the automatic signals fired for it.
///
/// Locks only ever ratchet on: an automatic lock (a sign-in from a locked
/// country, or from an IP shared with a locked account) locks an existing
/// account, and a sign-in from elsewhere never clears a lock that is already
/// there.
///
/// Paid and admin accounts are the exception — they are never locked
/// automatically. Both signals are coarse: a VPN, an office's shared NAT, or
/// someone travelling all trip them, and shutting a paying customer out on
/// that evidence costs more than letting the sign-in through. An admin can
/// still lock the account by hand, and `user.locked` carries that lock
/// through untouched.
///
/// Accounts created during sign-in are always Free, so this only ever spares
/// an account that already exists and already pays.
fn locked_after_sign_in(user: &User, auto_lock: bool) -> bool {
    user.locked || (auto_lock && !user.is_pro() && !user.is_admin())
}

#[cfg(test)]
mod tests {
    use super::locked_after_sign_in;
    use crate::domain::user::{AuthProvider, Role, User};
    use chrono::Utc;

    fn user(role: Role, locked: bool) -> User {
        User {
            id: 1,
            country_id: None,
            region_id: None,
            city_id: None,
            full_name: "Ada Lovelace".to_string(),
            first_name: "Ada".to_string(),
            last_name: Some("Lovelace".to_string()),
            email: "ada@example.com".to_string(),
            image_url: String::new(),
            role,
            verified: true,
            auth_provider: AuthProvider::Google,
            locked,
            plan_expiry_date: None,
            last_known_ip: "127.0.0.1".to_string(),
            stripe_customer_id: None,
            country_code: "unknown".to_string(),
            country_name: "Unknown".to_string(),
            region_name: None,
            city_name: None,
            last_seen_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn free_accounts_still_lock_automatically() {
        assert!(locked_after_sign_in(&user(Role::Free, false), true));
    }

    #[test]
    fn paid_and_admin_accounts_are_never_locked_automatically() {
        assert!(!locked_after_sign_in(
            &user(Role::Professional, false),
            true
        ));
        assert!(!locked_after_sign_in(&user(Role::Admin, false), true));
    }

    #[test]
    fn an_existing_lock_survives_every_sign_in() {
        for role in [Role::Free, Role::Professional, Role::Admin] {
            assert!(locked_after_sign_in(&user(role, true), false));
        }
    }

    #[test]
    fn a_clean_sign_in_never_locks_anyone() {
        assert!(!locked_after_sign_in(&user(Role::Free, false), false));
        assert!(!locked_after_sign_in(
            &user(Role::Professional, false),
            false
        ));
    }
}
