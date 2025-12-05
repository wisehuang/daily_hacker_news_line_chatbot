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

/// Safely truncate a string to max_chars characters without breaking UTF-8
fn truncate_string(s: &str, max_chars: usize) -> String {
    let char_count: usize = s.chars().count();

    if char_count <= max_chars {
        s.to_string()
    } else {
        // Take max_chars - 3 to leave room for "..."
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Create a text message for LINE
pub fn create_text_message(text: &str) -> LineMessage {
    LineMessage {
        message_type: "text".to_string(),
        text: Some(text.to_string()),
        alt_text: None,
        contents: None,
    }
}

/// Create a Flex Message carousel for stories with summaries
pub fn create_stories_carousel(stories: &[(crate::models::Story, String)]) -> LineMessage {
    use serde_json::json;

    // Create bubble for each story with its summary
    let bubbles: Vec<serde_json::Value> = stories
        .iter()
        .enumerate()
        .map(|(index, (story, summary))| {
            create_story_bubble(index + 1, &story.story, &story.storylink, summary)
        })
        .collect();

    LineMessage {
        message_type: "flex".to_string(),
        text: None,
        alt_text: Some("Today's Hacker News Stories".to_string()),
        contents: Some(json!({
            "type": "carousel",
            "contents": bubbles
        })),
    }
}

/// Create a single story bubble for the carousel
fn create_story_bubble(rank: usize, title: &str, link: &str, summary: &str) -> serde_json::Value {
    use serde_json::json;

    // Truncate title if too long (LINE has limits) - character-aware truncation
    let display_title = truncate_string(title, 100);

    // Truncate summary if too long - character-aware truncation
    let display_summary = truncate_string(summary, 200);

    json!({
        "type": "bubble",
        "size": "kilo",
        "header": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "box",
                    "layout": "baseline",
                    "contents": [
                        {
                            "type": "text",
                            "text": format!("#{}", rank),
                            "color": "#FF6B35",
                            "size": "sm",
                            "weight": "bold",
                            "flex": 0
                        },
                        {
                            "type": "text",
                            "text": "Hacker News",
                            "color": "#AAAAAA",
                            "size": "xs",
                            "margin": "md"
                        }
                    ]
                }
            ],
            "paddingAll": "15px",
            "backgroundColor": "#FFF5F0"
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": display_title,
                    "weight": "bold",
                    "size": "md",
                    "wrap": true,
                    "color": "#1A1A1A",
                    "margin": "none"
                },
                {
                    "type": "separator",
                    "margin": "md"
                },
                {
                    "type": "text",
                    "text": display_summary,
                    "size": "sm",
                    "wrap": true,
                    "color": "#666666",
                    "margin": "md"
                }
            ],
            "spacing": "md",
            "paddingAll": "15px"
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "button",
                    "action": {
                        "type": "uri",
                        "label": "Read Article",
                        "uri": link
                    },
                    "style": "primary",
                    "color": "#FF6B35",
                    "height": "sm"
                }
            ],
            "spacing": "sm",
            "paddingAll": "13px"
        }
    })
}

/// Create a Flex Message bubble for summary
pub fn create_summary_bubble(summary: &str) -> LineMessage {
    use serde_json::json;

    // Split summary into sections if it contains double newlines
    let sections: Vec<&str> = summary.split("\n\n").collect();

    // Create text components for each section
    let text_contents: Vec<serde_json::Value> = sections
        .iter()
        .enumerate()
        .flat_map(|(index, section)| {
            let mut components = vec![];

            // Add separator between sections (except first)
            if index > 0 {
                components.push(json!({
                    "type": "separator",
                    "margin": "xl"
                }));
            }

            // Add text with appropriate margin
            components.push(json!({
                "type": "text",
                "text": section.trim(),
                "wrap": true,
                "size": "sm",
                "color": "#333333",
                "margin": if index > 0 { "md" } else { "none" }
            }));

            components
        })
        .collect();

    LineMessage {
        message_type: "flex".to_string(),
        text: None,
        alt_text: Some("Today's Hacker News Summary".to_string()),
        contents: Some(json!({
            "type": "bubble",
            "size": "mega",
            "header": {
                "type": "box",
                "layout": "vertical",
                "contents": [
                    {
                        "type": "box",
                        "layout": "baseline",
                        "contents": [
                            {
                                "type": "text",
                                "text": "📰",
                                "size": "xl",
                                "flex": 0
                            },
                            {
                                "type": "text",
                                "text": "Today's Summary",
                                "weight": "bold",
                                "size": "xl",
                                "margin": "md",
                                "color": "#FFFFFF"
                            }
                        ]
                    },
                    {
                        "type": "text",
                        "text": chrono::Local::now().format("%Y-%m-%d").to_string(),
                        "size": "xs",
                        "color": "#FFFFFFCC",
                        "margin": "md"
                    }
                ],
                "paddingAll": "20px",
                "backgroundColor": "#FF6B35"
            },
            "body": {
                "type": "box",
                "layout": "vertical",
                "contents": text_contents,
                "spacing": "md",
                "paddingAll": "20px"
            }
        })),
    }
} 
