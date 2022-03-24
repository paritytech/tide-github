#![deny(missing_docs)]
//!
//! Process Github webhooks in [tide](https://github.com/http-rs/tide).
//!
//! ## Example
//!
//! ```Rust
//! #[async_std::main]
//! async fn main() -> tide::Result<()> {
//!     let mut app = tide::new();
//!     let github = tide_github::new(b"My Github webhook s3cr#t")
//!         .on(Event::IssueComment, |payload| {
//!             println!("Received a payload for repository {}", payload.repository.name);
//!         })
//!         .build();
//!     app.at("/gh_webhooks").nest(github);
//!     app.listen("127.0.0.1:3000").await?;
//!     Ok(())
//! }
//!
//! ```
//!
//! The API is still in development and may change in unexpected ways.
use async_trait::async_trait;
use std::collections::HashMap;
use tide::{prelude::*, Request, StatusCode};
use std::sync::Arc;

mod middleware;
mod payload;
use payload::Payload;

/// Returns a [`ServerBuilder`] with the given webhook secret.
///
/// Call [`Self::on()`](on@ServerBuilder) to register closures to be run when the given event is
/// received and [`Self::build()`](build@ServerBuilder) to retrieve the final [`tide::Server`].
pub fn new<S: Into<String>>(webhook_secret: S) -> ServerBuilder {
    ServerBuilder::new(webhook_secret.into())
}

type HandlerMap = HashMap<
    Event,
    // TODO: Create a nice type alias for the Event Handler
    Arc<dyn Send + Sync + 'static + Fn(Payload)>,
>;

/// [`ServerBuilder`] is used to first register closures to events before finally building a
/// [`tide::Server`] using those closures.
pub struct ServerBuilder {
    webhook_secret: String,
    handlers: HandlerMap,
}

impl ServerBuilder {
    fn new(webhook_secret: String) -> Self {
        ServerBuilder {
            webhook_secret,
            handlers: HashMap::new(),
        }
    }

    /// Registers the given event handler to be run when the given event is received.
    ///
    /// The event handler receives a [`Payload`] as the single argument. Since webhooks are
    /// generally passively consumed (Github will not meaningfully (to us) process our response),
    /// the handler returns only a `()`. As far as the event dispatcher is concerned, all the
    /// meaningful work will be done as side-effects of the closures you register here.
    ///
    /// The types involved here are not stable yet due to ongoing API development.
    ///
    /// ## Example
    ///
    /// ```Rust
    ///     let github = tide_github::new("my webhook s3ct#t")
    ///         .on(Event::IssueComment, |payload| {
    ///             println!("Got payload for repository {}", payload.repository.name)
    ///         });
    /// ```
    pub fn on<E: Into<Event>>(
        mut self,
        event: E,
        handler: impl Fn(Payload)
            + Send
            + Sync
            + 'static,
    ) -> Self {
        let event: Event = event.into();
        self.handlers.insert(event, Arc::new(handler));
        self
    }

    /// Build a [`tide::Server`] using the registered events.
    ///
    /// Since the API is still in development, in the future we might instead (or additionally)
    /// expose the `EventHandlerDispatcher` directly.
    pub fn build(self) -> tide::Server<()> {
        let mut server = tide::new();
        let dispatcher = Box::new(EventHandlerDispatcher::new(self.handlers));
        server.with(middleware::WebhookVerification::new(self.webhook_secret));
        server
            .at("/")
            .post(dispatcher as Box<dyn tide::Endpoint<()>>);
        server
    }
}

/// This enum represents the event (and its variants the event type) for which we can receive a
/// Github webhook.
///
/// Github sends the type of the event (and thus of the payload) as the `X-github-Event` header
/// that we parse into an `Event` by implementing [`::std::str::FromStr`] for it.
#[non_exhaustive]
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum Event {
    /// The Github
    /// [`IssueCommentEvent`](https://docs.github.com/en/developers/webhooks-and-events/events/github-event-types#issuecommentevent) event.
    IssueComment,
}

use std::fmt;
impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::IssueComment => write!(f, "issue_comment"),
        }
    }
}

impl ::std::str::FromStr for Event {
    type Err = EventDispatchError;

    fn from_str(event: &str) -> Result<Event, Self::Err> {
        use self::Event::*;

        // TODO: Generate this from a derive macro on `Event`
        match event {
            "issue_comment" => Ok(IssueComment),
            event => {
                log::warn!("Unsupported event: {}", event);
                Err(EventDispatchError::UnsupportedEvent)
            },
        }
    }
}

/// The variants of [`EventDispatchError`] represent the errors that would prevent us from calling
/// the handler to process the Github Webhook.
#[derive(thiserror::Error, Clone, Debug)]
pub enum EventDispatchError {
    /// Github send us a webhook for an [`Event`] that we don't support.
    #[error("Received an Event of an unsupported type")]
    UnsupportedEvent,
    /// We're processing something that does not seem to be a Github webhook.
    #[error("No `X-Github-Event` header found")]
    MissingEventHeader,
    /// No handler was registered for the event we received.
    #[error("No handler registered for Event '{0}'")]
    MissingHandlerForEvent(Event),
}

struct EventHandlerDispatcher {
    handlers: HandlerMap,
}

impl EventHandlerDispatcher {
    fn new(handlers: HandlerMap) -> Self {
        EventHandlerDispatcher { handlers }
    }
}

#[async_trait]
impl tide::Endpoint<()> for EventHandlerDispatcher
where
    EventHandlerDispatcher: Send + Sync,
{
    async fn call(&self, mut req: Request<()>) -> tide::Result {
        use std::str::FromStr;
        use async_std::task;

        let event_header = req
            .header("X-Github-Event")
            .ok_or(EventDispatchError::MissingEventHeader)
            .status(StatusCode::BadRequest)?.as_str();

        let event = Event::from_str(event_header).status(StatusCode::NotImplemented)?;
        let payload: payload::Payload = req.body_json().await?;
        let handler = self
            .handlers
            .get(&event)
            .ok_or_else(|| { println!("Missing Handler for Event {:?}", event); EventDispatchError::MissingHandlerForEvent(event)})
            .status(StatusCode::NotImplemented)?;

        let handler = handler.clone();

        task::spawn_blocking(move || {handler(payload)});

        Ok("".into())
    }
}
