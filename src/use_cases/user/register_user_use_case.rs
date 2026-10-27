use crate::domain::country::Country;
use crate::domain::region::Region;
use crate::domain::user::{AuthProvider, CreateUser, User, UserForm};
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

    /// Resolve the user's IP to a country/region, then either create the user
    /// or refresh their audit fields. Country `locked` propagates as a sign-in
    /// block.
    pub async fn execute(&self, request: &mut CreateUser) -> Result<User> {
        self.resolve_location(request).await?;

        match User::find_by_email(self.db.as_ref(), &request.email).await {
            Ok(user) => self.update_audit_fields(user, request).await,
            Err(_) => User::create(self.db.as_ref(), request).await,
        }
    }

    /// Fills in the country/region for `request.last_known_ip` (via `geodude`
    /// if configured), persisting any newly-seen rows, and locks the request
    /// if that country is locked.
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
        }

        if country.locked {
            request.locked = true;
        }

        Ok(())
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

        // Locks only ever ratchet on: a sign-in from a locked country locks an
        // existing account, and a later clean sign-in never clears the lock.
        let locked = user.locked || request.locked;

        let mut form: UserForm = user.into();
        form.locked = locked.then_some("on".into());
        form.last_seen_at = Utc::now();
        form.last_known_ip = request.last_known_ip.clone();
        form.country_id = request.country_id;
        form.region_id = request.region_id;

        User::update(self.db.as_ref(), user_id, &form).await
    }
}
