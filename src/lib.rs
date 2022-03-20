#![deny(missing_docs)]
//!
//! Process Github webhooks in [tide](https://github.com/http-rs/tide).
//!
//! ## Example
//!
//! ```Rust
//! use octocrab::models::issues::Comment;
//! use tide_github::Event;
//!
//! #[async_std::main]
//! async fn main() -> tide::Result<()> {
//!     let mut app = tide::new();
//!     let github = tide_github::new(b"My Github webhook s3cr#t")
//!         .on(Event::IssuesComment, |mut req| {
//!             Box::pin(async move {
//!                 let _comment: Comment = req.body_json().await.unwrap();
//!             })
//!         })
//!         .build();
//!     app.at("/gh_webhooks").nest(github);
//!     app.listen("127.0.0.1:3000").await?;
//!     Ok(())
//! }
//! ```
//!
//! The API is still in development and may change in unexpected ways.
use async_trait::async_trait;
use futures::future::Future;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tide::{prelude::*, Request, StatusCode};

mod middleware;

/// Returns a [`ServerBuilder`] with the given webhook secret.
///
/// Call [`Self::on()`](on@ServerBuilder) to register closures to be run when the given event is
/// received and [`Self::build()`](build@ServerBuilder) to retrieve the final [`tide::Server`].
pub fn new(webhook_secret: &'static [u8]) -> ServerBuilder {
    ServerBuilder::new(webhook_secret)
}

type HandlerMap = HashMap<
    Event,
    Arc<dyn Send + Sync + 'static + Fn(Request<()>) -> Pin<Box<dyn Future<Output = ()> + Send>>>,
>;

/// [`ServerBuilder`] is used to first register closures to events before finally building a
/// [`tide::Server`] using those closures.
pub struct ServerBuilder {
    webhook_secret: &'static [u8],
    handlers: HandlerMap,
}

impl ServerBuilder {
    fn new(webhook_secret: &'static [u8]) -> Self {
        ServerBuilder {
            webhook_secret,
            handlers: HashMap::new(),
        }
    }

    /// Registers the given event handler to be run when the given event is received.
    ///
    /// The event handler receives a [`tide::Request`] as the single argument. Because
    /// [`tide::Request`] is not really useful in synchronous (non-`async`) environments, the event
    /// handler itself is required to be `async`. Since webhooks are generally passively consumed
    /// (Github will not meaningfully (to us) process our response), the handler returns only a
    /// `()` in it's `Future`. As far as the event dispatcher is concerned, all the
    /// meaningful work will be done as side-effects of the closures you register here.
    ///
    /// The types involved here are not stable yet due to ongoing API development.
    ///
    /// ## Example
    ///
    /// ```Rust
    ///     use octocrab::models::issues::Comment;
    ///     let github = tide_github::new("my webhook s3ct#t")
    ///         .on(Event::IssuesComment, |mut req| {
    ///             Box::pin(async move {
    ///                 println!("Something happened with an issue comment");
    ///                 let _comment: Comment = req.body_json().await.unwrap();
    ///             })
    ///         });
    /// ```
    pub fn on<E: Into<Event>>(
        mut self,
        event: E,
        handler: impl Fn(Request<()>) -> Pin<Box<dyn Future<Output = ()> + Send>>
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
#[derive(PartialEq, Eq, Hash)]
pub enum Event {
    /// The Github
    /// [`IssueCommentEvent`](https://docs.github.com/en/developers/webhooks-and-events/events/github-event-types#issuecommentevent) event.
    IssuesComment,
}

impl ::std::str::FromStr for Event {
    type Err = EventDispatchError;

    fn from_str(event: &str) -> Result<Event, Self::Err> {
        use self::Event::*;

        // TODO: Generate this from a derive macro on `Event`
        match event {
            "issues_comment" => Ok(IssuesComment),
            event => Err(EventDispatchError::UnsupportedEvent(event.into())),
        }
    }
}

/// The variants of [`EventDispatchError`] represent the errors that would prevent us from calling
/// the handler to process the Github Webhook.
#[derive(thiserror::Error, Debug)]
pub enum EventDispatchError {
    /// Github send us a webhook for an [`Event`] that we don't support.
    #[error("Event '{0}' is not supported")]
    UnsupportedEvent(String),
    /// We're processing something that does not seem to be a Github webhook.
    #[error("No `X-Github-Event` header found")]
    MissingEventHeader,
    /// No handler was registered for the event we received.
    #[error("No handler registered for Event '{0}'")]
    MissingHandlerForEvent(String),
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
    async fn call(&self, req: Request<()>) -> tide::Result {
        use std::str::FromStr;

        let event_header = req
            .header("X-Github-Event")
            .ok_or(EventDispatchError::MissingEventHeader)
            .status(StatusCode::BadRequest)?;
        let event = Event::from_str(event_header.as_str()).status(StatusCode::NotImplemented)?;
        let handler = self
            .handlers
            .get(&event)
            .ok_or_else(|| EventDispatchError::MissingHandlerForEvent(event_header.as_str().into()))
            .status(StatusCode::NotImplemented)?;

        (handler)(req).await;

        Ok("".into())
    }
}
