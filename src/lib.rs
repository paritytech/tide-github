use async_trait::async_trait;
use futures::future::Future;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tide::{prelude::*, Request, StatusCode};

mod middleware;

pub fn new(webhook_secret: &'static [u8]) -> ServerBuilder {
    ServerBuilder::new(webhook_secret)
}

type HandlerMap = HashMap<
    Event,
    Arc<dyn Send + Sync + 'static + Fn(Request<()>) -> Pin<Box<dyn Future<Output = ()> + Send>>>,
>;

pub struct ServerBuilder {
    webhook_secret: &'static [u8],
    handlers: HandlerMap,
}

impl ServerBuilder {
    pub fn new(webhook_secret: &'static [u8]) -> Self {
        ServerBuilder {
            webhook_secret,
            handlers: HashMap::new(),
        }
    }

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

#[non_exhaustive]
#[derive(PartialEq, Eq, Hash)]
pub enum Event {
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

#[derive(thiserror::Error, Debug)]
pub enum EventDispatchError {
    #[error("Event '{0}' is not supported")]
    UnsupportedEvent(String),
    #[error("No `X-Github-Event` header found")]
    MissingEventHeader,
    #[error("No handler registered for Event '{0}'")]
    MissingHandlerForEvent(String),
}
