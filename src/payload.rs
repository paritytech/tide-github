use octocrab::models::{Repository, User, issues::Comment, issues::Issue};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Payload {
    pub action: Action,
    pub sender: User,
    pub repository: Repository,
    pub comment: Option<Comment>,
    pub issue: Option<Issue>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Created,
    Edited,
    Deleted,
}
