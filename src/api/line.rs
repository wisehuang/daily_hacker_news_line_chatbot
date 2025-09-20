use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use sha2::Sha256;
use bytes::Bytes;

use crate::models::{
    ApiError, ApiResult, LineBroadcastRequest, LineMessage, LineMessageRequest, LineSendMessageRequest,
};
use crate::utils::{HTTP_CLIENT, CONFIG_CACHE};

type HmacSha256 = Hmac<Sha256>;

/// Validate the LINE message signature using constant-time comparison
pub fn validate_signature(signature: String, body: &Bytes) -> ApiResult<()> {
    let channel_secret = &CONFIG_CACHE.channel_secret;
    
    let mut mac = HmacSha256::new_from_slice(channel_secret.as_bytes())
        .map_err(|_| ApiError::ValidationError("Invalid HMAC key length".to_string()))?;
    
    mac.update(body);
    let signature_bytes = general_purpose::STANDARD
        .decode(signature.as_bytes())
        .map_err(|_| ApiError::ValidationError("Invalid signature".to_string()))?;

    mac.verify_slice(&signature_bytes)
        .map_err(|_| ApiError::ValidationError("Invalid signature".to_string()))
}

/// Create common headers for LINE API requests
fn create_line_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    
    // Use try_from instead of from_str to handle invalid header value characters safely
    match HeaderValue::try_from(format!("Bearer {}", token)) {
        Ok(auth_value) => {
            headers.insert(AUTHORIZATION, auth_value);
        },
        Err(e) => {
            log::error!("Failed to create Authorization header: {}", e);
            // Continue with empty Authorization, which will fail at the API level
            // but won't cause a panic in our application
        }
    }
    
    headers
}

/// Send a push message to a specific user
pub async fn push_message(token: &str, user_id: &str, messages: Vec<LineMessage>) -> ApiResult<()> {
    let push_url = &CONFIG_CACHE.line_push_url;
    
    let request = LineMessageRequest {
        to: user_id.to_string(),
        messages,
    };
    
    let response = HTTP_CLIENT
        .post(push_url)
        .headers(create_line_headers(token))
        .json(&request)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(ApiError::ExternalServiceError(format!("LINE API error: {}", error_text)))
    }
}

/// Send a broadcast message to all users
pub async fn broadcast_message(token: &str, messages: Vec<LineMessage>) -> ApiResult<()> {
    let broadcast_url = &CONFIG_CACHE.line_broadcast_url;
    
    let request = LineBroadcastRequest { messages };
    
    let response = HTTP_CLIENT
        .post(broadcast_url)
        .headers(create_line_headers(token))
        .json(&request)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(ApiError::ExternalServiceError(format!("LINE broadcast error: {}", error_text)))
    }
}

/// Send a reply to a specific message
pub async fn reply_message(token: &str, reply_token: &str, messages: Vec<LineMessage>) -> ApiResult<()> {
    let reply_url = &CONFIG_CACHE.line_reply_url;
    
    let request = LineSendMessageRequest {
        reply_token: reply_token.to_string(),
        messages,
    };
    
    let response = HTTP_CLIENT
        .post(reply_url)
        .headers(create_line_headers(token))
        .json(&request)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(ApiError::ExternalServiceError(format!("LINE reply error: {}", error_text)))
    }
}

/// Create a text message for LINE
pub fn create_text_message(text: &str) -> LineMessage {
    LineMessage {
        message_type: "text".to_string(),
        text: text.to_string(),
    }
} 
