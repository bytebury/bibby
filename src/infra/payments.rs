#[derive(Clone)]
pub struct Stripe {
    pub client: shima::Client,
    pub listener: shima::webhook::Listener,
}

impl Default for Stripe {
    fn default() -> Self {
        let client = shima::Client::from_env();

        Self {
            client: client.clone(),
            listener: shima::webhook::Listener::new(client),
        }
    }
}
