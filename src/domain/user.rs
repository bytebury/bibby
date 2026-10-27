use crate::domain::Access;
use crate::domain::value_objects::email::Email;
use crate::infra::pagination::Paginate;
use crate::prelude::*;
use chrono::NaiveDate;

#[derive(Debug, Deserialize, Serialize, Clone, FromRow)]
pub struct User {
    pub id: PrimaryKey,
    pub country_id: Option<PrimaryKey>,
    pub region_id: Option<PrimaryKey>,
    pub city_id: Option<PrimaryKey>,
    pub full_name: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: String,
    pub image_url: String,
    pub role: Role,
    /// Whether the owner has proven they own `email`. Google sign-ups are
    /// verified by Google; email sign-ups stay unverified — and can't hold a
    /// session — until they enter the code we email them.
    pub verified: bool,
    pub auth_provider: AuthProvider,
    pub locked: bool,
    /// Last day an admin-granted Professional plan stays active (inclusive).
    /// `None` means the plan does not expire — either because it is billed
    /// through Stripe or because the admin granted it indefinitely.
    pub plan_expiry_date: Option<NaiveDate>,
    pub last_known_ip: String,
    pub stripe_customer_id: Option<String>,
    pub country_code: String,
    pub country_name: String,
    pub region_name: Option<String>,
    pub city_name: Option<String>,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The `users.email` column is `UNIQUE`, so pointing an account at an address
/// another account already uses trips constraint `users_email_key`. The
/// generic 23505 mapping only says "This record already exists.", which gives
/// an admin nothing to act on.
fn map_email_conflict(e: sqlx::Error) -> AppError {
    let constraint = e.as_database_error().and_then(|de| de.constraint());
    if constraint.is_some_and(|c| c == "users_email_key") {
        AppError::Conflict("Another account already uses that email address.".to_string())
    } else {
        AppError::from(e)
    }
}

impl User {
    pub async fn create<'e, E>(exec: E, request: &CreateUser) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres> + Clone,
    {
        let user_id: PrimaryKey = sqlx::query_scalar(
            r#"
            INSERT INTO users (full_name, first_name, last_name, email, image_url, role, verified, locked, last_known_ip, country_id, region_id, auth_provider, city_id)
            VALUES ($1, $2, $3, $4, $5, COALESCE((SELECT 'admin' WHERE NOT EXISTS (SELECT 1 FROM users)), $6), $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(&request.full_name)
        .bind(&request.first_name)
        .bind(&request.last_name)
        .bind(&request.email)
        .bind(&request.image_url)
        .bind(request.role.to_string())
        .bind(request.verified)
        .bind(request.locked)
        .bind(&request.last_known_ip)
        .bind(request.country_id)
        .bind(request.region_id)
        .bind(request.auth_provider.to_string())
        .bind(request.city_id)
        .fetch_one(exec.clone())
        .await?;

        User::find_by_id(exec, user_id).await
    }

    pub async fn find_by_email<'e, E>(exec: E, email: &str) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT
                id as "id!",
                country_id,
                region_id,
                city_id,
                full_name as "full_name!",
                first_name as "first_name!",
                last_name,
                email as "email!",
                image_url as "image_url!",
                role as "role!: Role",
                verified as "verified!",
                auth_provider as "auth_provider!: AuthProvider",
                locked as "locked!",
                plan_expiry_date,
                last_known_ip as "last_known_ip!",
                stripe_customer_id,
                country_code as "country_code!",
                country_name as "country_name!",
                region_name,
                city_name,
                last_seen_at as "last_seen_at!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_details WHERE email = $1
            "#,
            email,
        )
        .fetch_one(exec)
        .await?;
        Ok(user)
    }

    pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT
                id as "id!",
                country_id,
                region_id,
                city_id,
                full_name as "full_name!",
                first_name as "first_name!",
                last_name,
                email as "email!",
                image_url as "image_url!",
                role as "role!: Role",
                verified as "verified!",
                auth_provider as "auth_provider!: AuthProvider",
                locked as "locked!",
                plan_expiry_date,
                last_known_ip as "last_known_ip!",
                stripe_customer_id,
                country_code as "country_code!",
                country_name as "country_name!",
                region_name,
                city_name,
                last_seen_at as "last_seen_at!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_details WHERE id = $1
            "#,
            id,
        )
        .fetch_one(exec)
        .await?;
        Ok(user)
    }

    /// Per-request audit + lookup in a single statement. This runs on every
    /// authenticated request (see `BaseUser::from_request_parts`), so the CTE
    /// folds the lookup, the audit UPDATE, and the re-read into one round
    /// trip. Only the columns the audit actually changes (`last_known_ip`,
    /// `last_seen_at`) are written, which avoids racily writing back stale
    /// copies of `role`/`locked`/etc.
    ///
    /// The same CTE also records a (user_id, UTC day) row in
    /// `user_daily_activity` — at most one per user per day, thanks to the
    /// composite primary key and `ON CONFLICT DO NOTHING`. This feeds the Site
    /// Analytics DAU chart and the admin user page's activity strip.
    pub async fn audit_and_find_by_email<'e, E>(exec: E, email: &str, ip: &str) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as(
            r#"
            WITH audited AS (
                UPDATE users
                   SET last_known_ip = $2, last_seen_at = NOW()
                 WHERE email = $1
             RETURNING id
            ),
            _activity AS (
                INSERT INTO user_daily_activity (user_id, day)
                SELECT id, (NOW() AT TIME ZONE 'UTC')::DATE FROM audited
                ON CONFLICT DO NOTHING
            )
            SELECT ud.* FROM user_details ud JOIN audited ON audited.id = ud.id
            "#,
        )
        .bind(email)
        .bind(ip)
        .fetch_one(exec)
        .await?;
        Ok(user)
    }

    /// Admin edit from the manage-users modal. Only the fields that form edits
    /// are written: location and audit columns belong to the sign-in and
    /// per-request audit paths, and round-tripping them through the browser
    /// would write back a stale copy.
    pub async fn update<'e, E>(exec: E, id: PrimaryKey, request: &UserUpdate) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres> + Clone,
    {
        // An expiry date only means something for an admin-granted paid plan.
        // Free and Admin accounts have nothing to expire, so a stale date left
        // over from a previous role is dropped rather than kept around for
        // the scheduler to act on later.
        let plan_expiry_date = match request.role {
            Role::Professional => request.plan_expiry_date,
            Role::Free | Role::Admin => None,
        };

        sqlx::query!(
            r#"
            UPDATE users
               SET role = $1,
                   locked = $2,
                   plan_expiry_date = $3,
                   email = $4,
                   -- Moving the deadline re-arms the reminder ladder; the
                   -- CASE reads the pre-UPDATE value, so an unchanged date
                   -- keeps whatever reminders have already gone out.
                   plan_expiry_reminder_days = CASE
                       WHEN plan_expiry_date IS DISTINCT FROM $3 THEN NULL
                       ELSE plan_expiry_reminder_days
                   END
             WHERE id = $5
            "#,
            request.role.to_string(),
            request.locked,
            plan_expiry_date,
            request.email.as_str(),
            id,
        )
        .execute(exec.clone())
        .await
        .map_err(map_email_conflict)?;

        User::find_by_id(exec, id).await
    }

    /// Refreshes what a sign-in learns about an existing account: where it
    /// came from, when, and whether the automatic lock signals fired.
    pub async fn update_sign_in_fields<'e, E>(
        exec: E,
        id: PrimaryKey,
        request: &CreateUser,
        locked: bool,
    ) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres> + Clone,
    {
        sqlx::query!(
            r#"
            UPDATE users
               SET last_known_ip = $1,
                   last_seen_at = NOW(),
                   country_id = $2,
                   region_id = $3,
                   city_id = $4,
                   locked = $5
             WHERE id = $6
            "#,
            request.last_known_ip,
            request.country_id,
            request.region_id,
            request.city_id,
            locked,
            id,
        )
        .execute(exec.clone())
        .await?;

        User::find_by_id(exec, id).await
    }

    /// Drop an admin-granted expiry date without touching the role. Called
    /// whenever Stripe takes over the plan (checkout completed) or ends it
    /// (subscription deleted / payment failed) so the scheduler never
    /// downgrades an account that billing is now responsible for.
    pub async fn clear_plan_expiry<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            "UPDATE users SET plan_expiry_date = NULL, plan_expiry_reminder_days = NULL WHERE id = $1",
            id,
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Whether a locked account was last seen on `ip` — one of the automatic
    /// lock signals at sign-in.
    pub async fn any_locked_with_ip<'e, E>(exec: E, ip: &str) -> Result<bool>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM users WHERE locked = true AND last_known_ip = $1) AS "exists!""#,
            ip,
        )
        .fetch_one(exec)
        .await?;
        Ok(exists)
    }

    pub async fn destroy<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub async fn set_stripe_customer_id<'e, E>(
        exec: E,
        id: PrimaryKey,
        customer_id: &shima::CustomerId,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("UPDATE users SET stripe_customer_id = $1 WHERE id = $2")
            .bind(customer_id.as_ref())
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub async fn find_by_stripe_customer_id<'e, E>(exec: E, customer_id: &str) -> Result<User>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT
                id as "id!",
                country_id,
                region_id,
                city_id,
                full_name as "full_name!",
                first_name as "first_name!",
                last_name,
                email as "email!",
                image_url as "image_url!",
                role as "role!: Role",
                verified as "verified!",
                auth_provider as "auth_provider!: AuthProvider",
                locked as "locked!",
                plan_expiry_date,
                last_known_ip as "last_known_ip!",
                stripe_customer_id,
                country_code as "country_code!",
                country_name as "country_name!",
                region_name,
                city_name,
                last_seen_at as "last_seen_at!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_details WHERE stripe_customer_id = $1
            "#,
            customer_id,
        )
        .fetch_one(exec)
        .await?;
        Ok(user)
    }

    pub async fn set_role<'e, E>(exec: E, id: PrimaryKey, role: &Role) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role.to_string())
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    /// Marks the account as owning its address — called once someone has
    /// typed in a code we emailed to it.
    pub async fn mark_verified<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!("UPDATE users SET verified = TRUE WHERE id = $1", id)
            .execute(exec)
            .await?;
        Ok(())
    }

    /// Replaces the name on an email sign-up nobody has verified yet. Anyone
    /// can start a sign-up for any address, so until the owner proves it is
    /// theirs, the most recent sign-up form wins — otherwise a stranger could
    /// pick the name the real owner ends up with. Verified accounts are never
    /// touched.
    pub async fn update_unverified_name<'e, E>(
        exec: E,
        id: PrimaryKey,
        first_name: &str,
        last_name: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            r#"
            UPDATE users
               SET first_name = $1,
                   last_name = $2,
                   full_name = $1 || ' ' || $2
             WHERE id = $3
               AND verified = FALSE
            "#,
            first_name,
            last_name,
            id,
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Turns an email sign-up into a Google account once Google has proven
    /// who owns the address: it is verified and takes Google's name and photo.
    pub async fn convert_to_google<'e, E>(
        exec: E,
        id: PrimaryKey,
        request: &CreateUser,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            r#"
            UPDATE users
               SET auth_provider = 'google',
                   verified = TRUE,
                   full_name = COALESCE(NULLIF($1, ''), full_name),
                   first_name = COALESCE(NULLIF($2, ''), first_name),
                   last_name = COALESCE($3, last_name),
                   image_url = COALESCE(NULLIF($4, ''), image_url)
             WHERE id = $5
            "#,
            request.full_name,
            request.first_name,
            request.last_name,
            request.image_url,
            id,
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Email sign-ups that have never used Google sign in with an emailed
    /// code. Their first verified Google sign-in turns them into a Google
    /// account.
    pub fn signs_in_with_email(&self) -> bool {
        matches!(self.auth_provider, AuthProvider::Email)
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, Role::Admin)
    }

    pub fn is_free(&self) -> bool {
        matches!(self.role, Role::Free)
    }

    pub fn is_pro(&self) -> bool {
        matches!(self.role, Role::Professional)
    }

    pub fn ensure_can_read<T>(&self, entity: &T) -> Result<()>
    where
        Self: Access<T>,
    {
        if self.can_read(entity) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden".to_string()))
        }
    }

    pub fn ensure_can_write<T>(&self, entity: &T) -> Result<()>
    where
        Self: Access<T>,
    {
        if self.can_write(entity) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden".to_string()))
        }
    }
}

impl Access<User> for User {
    fn can_read(&self, entity: &User) -> bool {
        self.is_admin() || self.id == entity.id
    }
    fn can_write(&self, _: &User) -> bool {
        self.is_admin()
    }
}

impl Paginate for User {
    fn table_name() -> &'static str {
        "user_details"
    }
}

#[derive(Clone, Debug)]
pub struct CreateUser {
    pub country_id: Option<PrimaryKey>,
    pub region_id: Option<PrimaryKey>,
    pub city_id: Option<PrimaryKey>,
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub full_name: String,
    pub image_url: String,
    pub role: Role,
    pub verified: bool,
    pub auth_provider: AuthProvider,
    pub locked: bool,
    pub last_known_ip: String,
}

impl CreateUser {
    /// A sign-in request for an account that already exists, for sign-ins
    /// that don't hand back a fresh profile (an emailed code). Every profile
    /// field echoes what is stored, so running it through
    /// `RegisterUserUseCase` only refreshes the location and lock signals.
    pub fn from_existing(user: &User, last_known_ip: impl Into<String>) -> Self {
        Self {
            country_id: user.country_id,
            region_id: user.region_id,
            city_id: user.city_id,
            email: user.email.clone(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            full_name: user.full_name.clone(),
            image_url: user.image_url.clone(),
            role: user.role.clone(),
            verified: user.verified,
            auth_provider: user.auth_provider.clone(),
            locked: user.locked,
            last_known_ip: last_known_ip.into(),
        }
    }
}

/// How an account was created, and so how it signs in.
#[derive(Debug, Default, Deserialize, Serialize, sqlx::Type, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case", type_name = "TEXT")]
pub enum AuthProvider {
    /// Created through Google OAuth. May also sign in with an emailed code.
    #[default]
    Google,
    /// Created through the sign-up form and never signed in with Google. Signs
    /// in with an emailed code; a verified Google sign-in converts it to
    /// `Google`.
    Email,
}

impl Display for AuthProvider {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthProvider::Google => write!(f, "google"),
            AuthProvider::Email => write!(f, "email"),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, sqlx::Type, Clone, PartialOrd, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case", type_name = "TEXT")]
pub enum Role {
    #[default]
    Free,
    Professional,
    Admin,
}

impl From<String> for Role {
    fn from(value: String) -> Self {
        match value.as_str() {
            "admin" => Role::Admin,
            "professional" => Role::Professional,
            _ => Role::Free,
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Professional => write!(f, "professional"),
            Role::Free => write!(f, "free"),
        }
    }
}

/// The longest first or last name the sign-up form accepts.
pub const MAX_NAME_LEN: usize = 100;

/// What someone fills in to create an account without Google, as submitted.
/// Parse it into a `SignUp` before use.
#[derive(Debug, Clone, Deserialize)]
pub struct SignUpForm {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

/// A `SignUpForm` whose names are trimmed and present and whose address is
/// well-formed.
#[derive(Debug, Clone)]
pub struct SignUp {
    pub first_name: String,
    pub last_name: String,
    pub email: Email,
}

impl TryFrom<SignUpForm> for SignUp {
    type Error = AppError;

    fn try_from(form: SignUpForm) -> Result<Self> {
        Ok(SignUp {
            first_name: required_name(&form.first_name, "First name")?,
            last_name: required_name(&form.last_name, "Last name")?,
            email: Email::parse(&form.email)?,
        })
    }
}

fn required_name(raw: &str, field: &str) -> Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(format!("{field} is required.")));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "{field} must be {MAX_NAME_LEN} characters or fewer."
        )));
    }
    Ok(name.to_string())
}

/// The admin manage-users modal, as submitted. Parse it into a `UserUpdate`
/// before use.
#[derive(Deserialize)]
pub struct UserForm {
    /// The address the account signs in with. Admins can rewrite this to point
    /// an existing account at a different login — see `edit_user` in
    /// `src/infra/api/users.rs`.
    pub email: String,
    pub role: Role,
    pub locked: Option<String>,
    /// Blank means "no expiry" — the browser submits an empty string for an
    /// untouched `<input type="date">`.
    #[serde(default, deserialize_with = "deserialize_optional_naive_date")]
    pub plan_expiry_date: Option<NaiveDate>,
}

/// A `UserForm` whose email is well-formed and lowercased.
pub struct UserUpdate {
    pub email: Email,
    pub role: Role,
    pub locked: bool,
    pub plan_expiry_date: Option<NaiveDate>,
}

impl TryFrom<UserForm> for UserUpdate {
    type Error = AppError;

    fn try_from(form: UserForm) -> Result<Self> {
        Ok(UserUpdate {
            email: Email::parse(&form.email)?,
            role: form.role,
            locked: form.locked.is_some(),
            plan_expiry_date: form.plan_expiry_date,
        })
    }
}

fn deserialize_optional_naive_date<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(val) => NaiveDate::parse_from_str(val, "%Y-%m-%d")
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(first: &str, last: &str, email: &str) -> SignUpForm {
        SignUpForm {
            first_name: first.to_string(),
            last_name: last.to_string(),
            email: email.to_string(),
        }
    }

    #[test]
    fn sign_up_trims_names_and_folds_the_email() {
        let sign_up =
            SignUp::try_from(form("  Ada ", " Lovelace  ", "  Ada@Example.COM ")).unwrap();
        assert_eq!(sign_up.first_name, "Ada");
        assert_eq!(sign_up.last_name, "Lovelace");
        assert_eq!(sign_up.email.as_str(), "ada@example.com");
    }

    #[test]
    fn sign_up_requires_every_field() {
        let message = |form: SignUpForm| SignUp::try_from(form).unwrap_err().to_string();
        assert_eq!(
            message(form(" ", "Lovelace", "ada@example.com")),
            "First name is required."
        );
        assert_eq!(
            message(form("Ada", "", "ada@example.com")),
            "Last name is required."
        );
        assert_eq!(
            message(form("Ada", "Lovelace", "not-an-email")),
            "Enter a valid email address."
        );
        assert_eq!(
            message(form(
                &"a".repeat(MAX_NAME_LEN + 1),
                "Lovelace",
                "ada@example.com"
            )),
            "First name must be 100 characters or fewer."
        );
    }
}
