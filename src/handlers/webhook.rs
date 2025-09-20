use bytes::Bytes;
use serde_json::{json, Value};
use warp::{http::StatusCode, reject::Rejection, reply::Reply};

use crate::api::chatgpt;
use crate::api::line;
use crate::models::LineWebhookRequest;
use crate::utils::CONFIG_CACHE;

/// Handle incoming webhook requests from LINE
pub async fn parse_request_handler(
    x_line_signature: String,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    // First validate the signature and only proceed on success
    match validate_signature(x_line_signature, &body).await {
        Ok(()) => {
            // Clone the body for async processing
            let body_clone = body.clone();

            // Process the message asynchronously to return response quickly
            tokio::spawn(async move {
                process_request(body_clone).await;
            });

            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": true})),
                StatusCode::OK,
            ))
        }
        Err(e) => {
            log::error!("Signature validation failed: {}", e);
            let error_msg = json!({"success": false, "error": "Invalid signature"});
            Ok(warp::reply::with_status(
                warp::reply::json(&error_msg),
                StatusCode::UNAUTHORIZED,
            ))
        }
    }
}

/// Validate the LINE message signature
async fn validate_signature(
    x_line_signature: String,
    body: &Bytes,
) -> Result<(), String> {
    match line::validate_signature(x_line_signature, body) {
        Ok(_) => Ok(()),
        Err(e) => {
            log::error!("Invalid signature: {:?}", e);
            Err("Invalid signature".to_string())
        }
    }
}

/// Process the webhook request
async fn process_request(body: Bytes) {
    // Get the channel token from cache
    let channel_token = &CONFIG_CACHE.channel_token;

    // Parse the request body into structured data
    let webhook_request: LineWebhookRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(e) => {
            log::error!("Failed to parse webhook body: {}", e);
            return;
        }
    };

    // Get the first event
    let event = match webhook_request.events.first() {
        Some(event) => event,
        None => {
            log::warn!("No events in webhook payload");
            return;
        }
    };

    // Only process message events
    if event.event_type != "message" {
        log::info!("Received non-message event: {}", event.event_type);
        return;
    }

    // Extract message text
    let text = match &event.message {
        Some(message) => match &message.text {
            Some(text) if !text.is_empty() => text.clone(),
            _ => {
                log::info!("Received message without text content");
                return;
            }
        },
        None => {
            log::info!("Received event without message");
            return;
        }
    };

    // Get language code for the message
    let language_code = match chatgpt::get_language_code(text.clone()).await {
        Ok(code) => code,
        Err(e) => {
            log::error!("Failed to get language code: {:?}", e);
            "en".to_string()
        }
    };

    // Extract tokens from the structured event
    let reply_token = event.reply_token.as_deref();
    let user_id = event.source.user_id.as_deref();

    // Process with ChatGPT
    let chatgpt_response = match chatgpt::run_conversation(text).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("ChatGPT error: {:?}", e);
            return;
        }
    };

    // Parse the function call
    let function_call: Value = match serde_json::from_str(&chatgpt_response) {
        Ok(value) => value,
        Err(e) => {
            log::error!("Failed to parse ChatGPT response: {}", e);
            return;
        }
    };

    log::info!("Function call: {}", function_call);

    // Handle the function call
    function_call_handler(
        function_call,
        channel_token,
        reply_token,
        user_id,
        &language_code,
    )
    .await;
}

/// Handle the function call from ChatGPT
async fn function_call_handler(
    function_call: Value,
    channel_token: &str,
    reply_token: Option<&str>,
    user_id: Option<&str>,
    language_code: &str,
) {
    let function_name = function_call.get("name").and_then(Value::as_str);

    match function_name {
        Some("reply_latest_story") => {
            if let Some(reply_token) = reply_token {
                handle_reply_latest_story(channel_token, reply_token).await;
            }
        }
        Some("push_summary") => {
            if let Some(user_id) = user_id {
                handle_push_summary(channel_token, user_id, language_code, &function_call).await;
            }
        }
        Some("push_url_summary") => {
            if let Some(user_id) = user_id {
                handle_push_url_summary(channel_token, user_id, language_code, &function_call).await;
            }
        }
        _ => {
            // Default: push message with the content
            if let Some(user_id) = user_id {
                handle_push_messages(channel_token, user_id, &function_call).await;
            }
        }
    }
}

/// Handle reply with latest story
async fn handle_reply_latest_story(channel_token: &str, reply_token: &str) {
    match crate::handlers::stories::get_latest_story_message().await {
        Ok(message) => {
            let messages = vec![message];
            if let Err(e) = line::reply_message(channel_token, reply_token, messages).await {
                log::error!("Failed to reply with latest story: {:?}", e);
            }
        }
        Err(e) => {
            log::error!("Error getting latest story: {:?}", e);
        }
    }
}

/// Handle push summary of selected stories
async fn handle_push_summary(
    channel_token: &str,
    user_id: &str,
    language_code: &str,
    function_call: &Value,
) {
    let arguments = function_call["arguments"].as_str().unwrap_or("{}");
    let parsed_args: Result<Value, _> = serde_json::from_str(arguments);
    
    if let Ok(args) = parsed_args {
        if let Some(indexes) = args["indexes"].as_array() {
            let indexes: Vec<usize> = indexes
                .iter()
                .filter_map(|i| i.as_u64().map(|n| n as usize))
                .collect();
                
            if !indexes.is_empty() {
                if let Err(e) = crate::handlers::stories::push_story_summaries(
                    channel_token, 
                    user_id, 
                    language_code, 
                    &indexes
                ).await {
                    log::error!("Failed to push story summaries: {:?}", e);
                }
            }
        }
    }
}

/// Handle URL summary
async fn handle_push_url_summary(
    channel_token: &str,
    user_id: &str,
    language_code: &str,
    function_call: &Value,
) {
    let arguments = function_call["arguments"].as_str().unwrap_or("{}");
    let parsed_args: Result<Value, _> = serde_json::from_str(arguments);
    
    if let Ok(args) = parsed_args {
        if let Some(url) = args["url"].as_str() {
            if let Err(e) = crate::handlers::stories::push_url_summary(
                channel_token,
                user_id,
                language_code,
                url,
            ).await {
                log::error!("Failed to push URL summary: {:?}", e);
            }
        }
    }
}

/// Handle simple message push
async fn handle_push_messages(channel_token: &str, user_id: &str, function_call: &Value) {
    if let Some(message) = function_call["message"].as_str() {
        let messages = vec![line::create_text_message(message)];
        
        if let Err(e) = line::push_message(channel_token, user_id, messages).await {
            log::error!("Failed to push message: {:?}", e);
        }
    }
} 
