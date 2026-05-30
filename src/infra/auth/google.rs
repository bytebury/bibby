use crate::domain::user::{CreateUser, Role};
use crate::infra::auth::OAuthProvider;
use crate::infra::auth::oauth_state::OAuthState;
use crate::prelude::*;
use oauth2::basic::BasicClient;
use oauth2::*;

#[derive(Deserialize)]
pub struct GoogleUser {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: String,
    pub picture: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub locale: Option<String>,
}

#[derive(Default)]
pub struct GoogleOAuth {}

impl GoogleOAuth {
    pub fn new() -> Self {
        GoogleOAuth {}
    }

    fn client(
        &self,
    ) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet, EndpointSet> {
        let client_id = ClientId::new(
            env::var("GOOGLE_CLIENT_ID")
                .expect("Missing the GOOGLE_CLIENT_ID environment variable."),
        );
        let client_secret = ClientSecret::new(
            env::var("GOOGLE_CLIENT_SECRET")
                .expect("Missing the GOOGLE_CLIENT_SECRET environment variable."),
        );
        let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
            .expect("Invalid auth endpoint URL.");
        let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
            .expect("Invalid token endpoint URL.");
        let redirect_url = RedirectUrl::new(
            env::var("GOOGLE_CALLBACK_URL")
                .expect("GOOGLE_CALLBACK_URL not provided")
                .to_string(),
        )
        .expect("Invalid redirect URL");
        let revocation_url = RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())
            .expect("Invalid revocation endpoint URL");

        BasicClient::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url)
            .set_revocation_url(revocation_url)
    }

    /// Build the authorize URL with a signed `state` that encodes the env this
    /// flow should land back on. The single registered `GOOGLE_CALLBACK_URL`
    /// (prod) is used as the redirect_uri for every env — the state is what
    /// lets the proxy decide where to forward the code.
    pub fn auth_url_for_target(&self, target: &str, nonce: &str) -> Result<String> {
        let state = OAuthState::sign(target.to_string(), nonce.to_string())?;
        let (url, _) = self
            .client()
            .authorize_url(|| CsrfToken::new(state))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.profile".to_string(),
            ))
            .url();
        Ok(url.as_str().to_string())
    }

    async fn fetch_google_user_info(&self, token: &str) -> Result<GoogleUser> {
        let client = reqwest::Client::new();
        let google_user = client
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json::<GoogleUser>()
            .await?;
        Ok(google_user)
    }
}

impl OAuthProvider for GoogleOAuth {
    fn get_auth_url(&self) -> String {
        let (authorize_url, _csrf_state) = self
            .client()
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.profile".to_string(),
            ))
            .url();
        authorize_url.as_str().to_string()
    }

    async fn exchange_code_for_user(&self, code: &str) -> Result<CreateUser> {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let token_result = self
            .client()
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(&http_client)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let access_token = token_result.access_token().secret().clone();
        let google_user = self.fetch_google_user_info(&access_token).await?;

        Ok(CreateUser::from(google_user))
    }
}

impl From<GoogleUser> for CreateUser {
    fn from(google_user: GoogleUser) -> Self {
        Self {
            email: google_user.email,
            verified: google_user.email_verified,
            first_name: google_user.given_name.unwrap_or(google_user.name.clone()),
            last_name: google_user.family_name,
            full_name: google_user.name,
            image_url: google_user.picture,
            role: Role::Free,
            locked: false,
            country_id: None,
            region_id: None,
            last_known_ip: "".to_string(),
        }
    }
}
