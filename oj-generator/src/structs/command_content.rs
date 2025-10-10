use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MessageContent {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct StringContent(pub String);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ConfigResponse {
    Success { data: serde_json::Value },
    Error { error: String },
}
