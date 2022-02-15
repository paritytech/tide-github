pub(crate) struct WebhookVerification {
    secret: &'static [u8],
}

impl WebhookVerification {
    pub(crate) fn new(secret: &'static [u8]) -> Self {
        WebhookVerification { secret }
    }
}

#[tide::utils::async_trait]
impl<State> tide::Middleware<State> for WebhookVerification
where
    State: Clone + Send + Sync + 'static,
{
    async fn handle(
        &self,
        mut req: tide::Request<State>,
        next: tide::Next<'_, State>,
    ) -> tide::Result {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        use tide::{Response, StatusCode};

        if let Some(value) = req.header("X-Hub-Signature-256") {
            let encoded_sig = value.to_owned();
            let signature = match encoded_sig.as_str().strip_prefix("sha256=") {
                Some(hex) => hex::decode(hex)?,
                None => {
                    log::warn!("Failed to verify Github's signature: Unexpected format");
                    return Ok(Response::new(StatusCode::BadRequest));
                }
            };
            let mut mac: Hmac<Sha256> = Hmac::new_from_slice(&self.secret)?;
            mac.update(&req.body_bytes().await?);
            if let Err(err) = mac.verify_slice(&signature) {
                log::warn!("Failed to verify Github's signature: {}", err);
                return Ok(Response::new(StatusCode::BadRequest));
            } else {
                let res = next.run(req).await;
                Ok(res)
            }
        } else {
            log::warn!("Event not signed but webhook secret configured, ignoring event");
            return Ok(Response::new(StatusCode::BadRequest));
        }
    }
}
