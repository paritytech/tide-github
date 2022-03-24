pub(crate) struct WebhookVerification {
    secret: String,
}

impl WebhookVerification {
    pub(crate) fn new(secret: String) -> Self {
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
                Some(hex) => match hex::decode(hex) {
                    Ok(hex) => hex,
                    Err(err) => {
                        log::warn!("Failed to hex decode Github's signature: {}", err);
                        return Ok(Response::new(StatusCode::BadRequest))
                    }
                },
                None => {
                    log::warn!("Failed to verify Github's signature: Unexpected format");
                    return Ok(Response::new(StatusCode::BadRequest));
                }
            };
            let mut mac: Hmac<Sha256> = Hmac::new_from_slice(&self.secret.as_bytes())?;
            let body = req.body_bytes().await?;
            mac.update(&body);
            req.set_body(body);
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
