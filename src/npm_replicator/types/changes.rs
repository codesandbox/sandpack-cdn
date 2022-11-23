use serde::{Deserialize, Serialize};

use super::document::RegistryDocument;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(untagged)]
pub enum Event {
    Change(ChangeEvent),
    Finished(FinishedEvent),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ChangeEvent {
    pub seq: serde_json::Value,
    pub id: String,
    pub changes: Vec<Change>,

    #[serde(default)]
    pub deleted: bool,

    #[serde(default)]
    pub doc: Option<RegistryDocument>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Change {
    pub rev: String,
}

// Don't think we actually need this but couch_rs uses it so let's roll with it
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct FinishedEvent {
    pub last_seq: serde_json::Value,
    pub pending: Option<u64>, // not available on CouchDB 1.0
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ChangesPage {
    pub results: Vec<Event>,
    pub last_seq: i64,
}
