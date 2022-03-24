use std::convert::TryInto;
use octocrab::models::{Repository, User, issues::Comment, issues::Issue};
use serde::Deserialize;

/// [`Payload`] represents the (JSON) payload of the webhook Github send us.
///
/// Every webhook includes this payload. The presence of the `Option`al fields
/// depends on the event type this payload was send for.
///
/// For some event types there exists a specialized type with `Option<T>`
/// changed for `T` where possible. Conversion from [`Payload`] to a more
/// specialized type can be done through `TryInto` implementations.
#[derive(Deserialize, Debug)]
pub struct Payload {
    /// The action (created/edited/deleted) that triggered the webhook.
    pub action: Action,
    /// The account that triggered the action that triggered the webhook.
    pub sender: User,
    /// The repository associated with the webhook.
    pub repository: Repository,
    /// The comment involved in the action. Only present for some event types.
    pub comment: Option<Comment>,
    /// The issue involved in the action. Only present for some event types.
    pub issue: Option<Issue>,
}

/// [`IssueCommentPayload`] is a specialized version of [`Payload`] for the
/// `IssueComment` event.
///
/// Notably, the `Option<T>` fields on [`Payload`] that should always be present
/// in the case of this event are now `T`. Because conversion can fail in case
/// the fields on the original [`Payload`] are `None`, conversion happens
/// through the [`TryInto`](::std::convert::TryInto) trait.
pub struct IssueCommentPayload {
    /// The action (created/edited/deleted) that triggered the webhook.
    pub action: Action,
    /// The account that triggered the action that triggered the webhook.
    pub sender: User,
    /// The issue the comment was placed on.
    pub issue: Issue,
    /// The comment involved in the action.
    pub comment: Comment,
    /// The repository the issue belongs to.
    pub repository: Repository,
}

impl TryInto<IssueCommentPayload> for Payload {
    type Error = Error;

    fn try_into(self) -> Result<IssueCommentPayload, Self::Error> {
        let comment = self.comment.ok_or(Error::MissingCommentPayload)?;
        let issue = self.issue.ok_or(Error::MissingIssuePayload)?;

        Ok(IssueCommentPayload {
            action: self.action,
            sender: self.sender,
            repository: self.repository,
            comment,
            issue,
        })
    }

}

/// The errors that can occur interpreting the webhook payload.
#[derive(thiserror::Error, Clone, Debug)]
pub enum Error {
    /// The event type indicated in the `X-Github-Event` header should include
    /// this field in the webhook payload but it didn't.
    #[error("Expected field \"comment\" not found in webhook payload")]
    MissingCommentPayload,
    /// The event type indicated in the `X-Github-Event` header should include
    /// this field in the webhook payload but it didn't.
    #[error("Expected field \"issue\" not found in webhook payload")]
    MissingIssuePayload,
}

/// Action represents the action the Github webhook is send for.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// The something was created.
    Created,
    /// The something has been edited.
    Edited,
    /// The something has been deleted.
    Deleted,
}
