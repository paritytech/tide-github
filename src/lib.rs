use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use tide::{prelude::*, Request, StatusCode};

mod middleware;

pub fn new(webhook_secret: &'static [u8]) -> ServerBuilder {
    ServerBuilder::new(webhook_secret)
}

type EventHandler = &'static (dyn Fn() -> tide::Result + Send + Sync);

pub struct ServerBuilder {
    webhook_secret: &'static [u8],
    handlers: HashMap<Event, EventHandler>,
}

impl ServerBuilder {
    pub fn new(webhook_secret: &'static [u8]) -> Self {
        ServerBuilder{webhook_secret, handlers: HashMap::new()}
    }

    pub fn on(mut self, event: Event, handler: EventHandler) -> Self {
        self.handlers.insert(event, handler);
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
    handlers: HashMap<Event, EventHandler>,
}

impl EventHandlerDispatcher {
    fn new(handlers: HashMap<Event, EventHandler>) -> Self {
        EventHandlerDispatcher { handlers }
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for EventHandlerDispatcher
where
    State: Clone + Send + Sync + 'static,
    EventHandlerDispatcher: Send + Sync + 'static,
{
    async fn call(&self, req: Request<State>) -> tide::Result {
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
        (handler)()
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
