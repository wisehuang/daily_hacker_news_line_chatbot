use serde::{Deserialize, Serialize};

/// Story structure representing a news item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub storylink: String,
    pub story: String,
}

/// Error types for API responses
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("AI service error: {0}")]
    AiError(String),
    
    #[error("External service error: {0}")]
    ExternalServiceError(String),
}

/// API result type
pub type ApiResult<T> = Result<T, ApiError>;

/// LINE API message model (supports both text and flex messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineMessage {
    #[serde(rename = "type")]
    pub message_type: String,

    // Text message fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    // Flex message fields
    #[serde(skip_serializing_if = "Option::is_none", rename = "altText")]
    pub alt_text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<serde_json::Value>,
}

/// Request structure for sending messages to LINE
#[derive(Debug, Serialize, Deserialize)]
pub struct LineMessageRequest {
    pub to: String,
    pub messages: Vec<LineMessage>,
}

/// Request structure for broadcasting to LINE
#[derive(Debug, Serialize, Deserialize)]
pub struct LineBroadcastRequest {
    pub messages: Vec<LineMessage>,
}

/// Request structure for replying to LINE messages
#[derive(Debug, Serialize, Deserialize)]
pub struct LineSendMessageRequest {
    pub reply_token: String,
    pub messages: Vec<LineMessage>,
}

/// Web response helpers
#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl ErrorResponse {
    pub fn new(error: &str) -> Self {
        Self {
            success: false,
            error: error.to_string(),
        }
    }
}

/// LINE webhook event structures
#[derive(Debug, Deserialize)]
pub struct LineWebhookRequest {
    pub events: Vec<LineWebhookEvent>,
}

#[derive(Debug, Deserialize)]
pub struct LineWebhookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "replyToken")]
    pub reply_token: Option<String>,
    pub source: LineEventSource,
    pub message: Option<LineEventMessage>,
}

#[derive(Debug, Deserialize)]
pub struct LineEventSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LineEventMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub text: Option<String>,
} 