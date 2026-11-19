#[derive(Clone)]
pub struct Stripe {
    pub client: shima::Client,
    pub listener: shima::webhook::Listener,
}

impl Stripe {
    /// The Stripe client, or `None` when `STRIPE_SECRET_KEY` is missing or
    /// empty so the app can run without billing.
    pub fn from_env() -> Option<Self> {
        match shima::Client::try_from_env() {
            Ok(client) => Some(Self {
                client: client.clone(),
                listener: shima::webhook::Listener::new(client),
            }),
            Err(_) => {
                tracing::info!("STRIPE_SECRET_KEY is not set; payments are off");
                None
            }
        }
    }
}
