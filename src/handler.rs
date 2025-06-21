use bytes::Bytes;
use serde_json::{json, Value};
use warp::{
    http::{Response, StatusCode},
    Rejection, Reply,
};
use warp::hyper::Body;

use crate::{chatgpt, config_helper, errors::AppError, kagi, line_helper, readrss, request_handler};
use crate::config_helper::{get_config, get_secret};
use crate::line_helper::{
    LineBroadcastRequest, LineMessage, LineMessageRequest, LineSendMessageRequest,
};

pub async fn conversation_handler(content: Bytes) -> Result<impl Reply, Rejection> {
    let conversions = String::from_utf8(content.to_vec())
        .map_err(|e| warp::reject::custom(AppError::InvalidUtf8(e)))?;
    let res = match chatgpt::run_conversation(conversions).await {
        Ok(result) => result,
        Err(e) => {
            log::error!("ChatGPT conversation failed: {}", e);
            return Err(warp::reject::custom(AppError::ChatGpt(e.to_string())));
        }
    };

    let function_call: Value = match serde_json::from_str(res.as_str()) {
        Ok(value) => value,
        Err(e) => {
            log::error!("Failed to parse function call JSON: {}", e);
            return Err(warp::reject::custom(AppError::JsonParse(e)));
        }
    };

    log::info!("function_call: {}", function_call);

    match function_call.get("name").and_then(Value::as_str) {
        Some(function_name) => {
            let arguments_value = match function_call["arguments"].as_str() {
                Some(args) => args,
                None => {
                    log::error!("Missing arguments in function call");
                    return Err(warp::reject::custom(AppError::MissingField("arguments".to_string())));
                }
            };
            let arguments: Value = match serde_json::from_str(arguments_value) {
                Ok(args) => args,
                Err(e) => {
                    log::error!("Failed to parse function arguments: {}", e);
                    return Err(warp::reject::custom(AppError::JsonParse(e)));
                }
            };

            log::info!("arguments: {}", arguments);

            if function_name == "push_summary" {
                if let Some(index) = arguments["indexes"].as_array() {
                    log::info!("index: {:?}", index);
                } else {
                    log::warn!("Missing or invalid indexes in push_summary arguments");
                }
            }

            let response = warp::reply::json(&json!(function_call));
            Ok(warp::reply::with_status(response, StatusCode::OK))
        }
        None => {
            let message = function_call["message"].as_str().unwrap_or("No message available");
            let response = warp::reply::json(&json!({
                "message": message,
            }));
            Ok(warp::reply::with_status(response, StatusCode::OK))
        }
    }
}

pub async fn parse_request_handler(
    x_line_signature: String,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    let validation_result = validate_signature(x_line_signature, &body).await;

    match validation_result {
        Ok(()) => {
            // Only spawn async task after successful validation
            // This allows us to move body directly without cloning
            tokio::spawn(async move {
                process_request(body).await;
            });

            // Immediately return HTTP 200 OK after signature validation
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": true})),
                StatusCode::OK,
            ))
        },
        Err(_e) => {
            // Body is dropped here without cloning since validation failed
            let error_msg = json!({"success": false, "error": "Invalid signature"});
            Ok(warp::reply::with_status(
                warp::reply::json(&error_msg),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

async fn validate_signature(
    x_line_signature: String,
    body: &Bytes,
) -> Result<(), &'static str> {
    match line_helper::is_signature_valid(x_line_signature, body) {
        Ok(_) => Ok(()),
        Err(_e) => {
            log::error!("Invalid signature");
            Err("Invalid signature")
        }
    }
}

async fn process_request(body: Bytes) {
    // Get the channel token from the configuration file
    let channel_token = get_secret("channel.token");

    // Parse the body as a LineWebhookRequest
    let json_value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => {
            log::error!("Failed to parse JSON body: {}", e);
            return;
        }
    };

    // Extract the text from the first message
    let text = json_value["events"]
        .get(0)
        .and_then(|event| event["message"].get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default()
        .to_string();

    let language_code = match chatgpt::get_language_code(text.to_owned()).await {
        Ok(code) => code,
        Err(e) => {
            log::error!("Failed to get language code: {}", e);
            "en".to_string() // Default fallback
        }
    };

    let reply_token = json_value["events"][0]["replyToken"].as_str();

    let user_id = json_value["events"][0]["source"]["userId"].as_str();

    let res = match chatgpt::run_conversation(text).await {
        Ok(result) => result,
        Err(e) => {
            log::error!("ChatGPT conversation failed: {}", e);
            return;
        }
    };

    let function_call: Value = match serde_json::from_str(res.as_str()) {
        Ok(value) => value,
        Err(e) => {
            log::error!("Failed to parse function call JSON: {}", e);
            return;
        }
    };

    log::info!("function_call: {}", function_call);

    function_call_handler(
        function_call,
        channel_token,
        reply_token,
        user_id,
        language_code,
    )
        .await;
}

async fn function_call_handler(
    function_call: Value,
    channel_token: String,
    reply_token: Option<&str>,
    user_id: Option<&str>,
    language_code: String,
) {
    let function_name = function_call.get("name").and_then(Value::as_str);

    match function_name {
        Some("reply_latest_story") => {
            if let Some(token) = reply_token {
                handle_reply_latest_story(&channel_token, &token.to_string()).await;
            } else {
                log::error!("Missing reply token for reply_latest_story");
            }
        }
        Some("push_summary") => {
            if let Some(uid) = user_id {
                handle_push_summary(&channel_token, uid, language_code, &function_call).await;
            } else {
                log::error!("Missing user ID for push_summary");
            }
        }
        Some("push_url_summary") => {
            if let Some(uid) = user_id {
                handle_push_url_summary(&channel_token, uid, "zh-tw".to_string(), &function_call).await;
            } else {
                log::error!("Missing user ID for push_url_summary");
            }
        }
        _ => {
            if let Some(uid) = user_id {
                handle_push_messages(&channel_token, uid, &function_call).await;
            } else {
                log::error!("Missing user ID for push_messages");
            }
        }
    }
}

async fn handle_reply_latest_story(channel_token: &str, reply_token: &str) {
    match reply_latest_story(channel_token, reply_token).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error reply latest story").await;
        }
    }
}

async fn handle_push_summary(channel_token: &str, user_id: &str, language_code: String, function_call: &Value) {
    let arguments_str = match function_call["arguments"].as_str() {
        Some(args) => args,
        None => {
            log::error!("Missing arguments in function call");
            return;
        }
    };
    
    let arguments: Value = match serde_json::from_str(arguments_str) {
        Ok(args) => args,
        Err(e) => {
            log::error!("Failed to parse function arguments: {}", e);
            return;
        }
    };
    
    let indexes = match arguments.get("indexes").and_then(Value::as_array) {
        Some(arr) => arr.iter()
            .filter_map(|i| i.as_u64().map(|u| u as usize))
            .collect::<Vec<usize>>(),
        None => {
            log::error!("Missing or invalid indexes in arguments");
            return;
        }
    };

    match push_summary(channel_token, user_id, language_code, indexes).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error push summary").await;
        }
    }
}

async fn handle_push_messages(channel_token: &str, user_id: &str, function_call: &Value) {
    let message = match function_call["message"].as_str() {
        Some(msg) => msg.to_string(),
        None => {
            log::error!("Missing message in function call");
            return;
        }
    };
    
    match push_messages(
        channel_token,
        user_id,
        vec![message],
    ).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error push messages").await;
        }
    }
}

async fn handle_push_url_summary(channel_token: &str, user_id: &str, language_code: String, function_call: &Value) {
    let arguments_str = match function_call.get("arguments").and_then(|v| v.as_str()) {
        Some(args) => args,
        None => {
            log::error!("Missing arguments in push_url_summary function call");
            return;
        }
    };
    
    let arguments_json: Value = match serde_json::from_str(arguments_str) {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to parse push_url_summary arguments: {}", e);
            return;
        }
    };
    
    let url = match arguments_json.get("url").and_then(|v| v.as_str()) {
        Some(url_str) => url_str.to_string(),
        None => {
            log::error!("Missing URL in push_url_summary arguments");
            return;
        }
    };
    match push_url_summary(channel_token, user_id, language_code, url).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error push url summary").await;
        }
    }
}

async fn handle_error_response(error: &str) -> Response<Body> {
    let error_msg = json!({"success": false, "error": error});
    warp::reply::with_status(
        warp::reply::json(&error_msg),
        StatusCode::INTERNAL_SERVER_ERROR,
    ).into_response()
}

pub async fn get_latest_stories() -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    Ok(warp::reply::json(&stories))
}

pub async fn get_latest_title() -> Result<impl Reply, Rejection> {
    let channel = match readrss::read_feed().await {
        Ok(ch) => ch,
        Err(_) => {
            return Err(warp::reject::custom(AppError::Config("Error fetching feed".to_string())));
        }
    };

    let latest_item = match readrss::get_latest_item(&channel) {
        Some(item) => item,
        None => {
            return Err(warp::reject::custom(AppError::Config("No items in feed".to_string())));
        }
    };

    let latest_title = latest_item.title().unwrap_or("Untitled item").to_string();

    let response = match Response::builder()
        .header("content-type", "text/plain")
        .status(StatusCode::OK)
        .body(Bytes::from(latest_title)) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(warp::reject::custom(AppError::Config("Failed to build response".to_string())));
        }
    };

    Ok(response)
}

fn reply_error_msg(error: &'static str, status: StatusCode) -> Response<Bytes> {
    let error_msg = Bytes::from(error);
    Response::builder()
        .header("content-type", "text/plain")
        .status(status)
        .body(error_msg)
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Bytes::from("Internal server error"))
                .unwrap()
        })
}

pub async fn send_line_broadcast() -> Result<impl Reply, Rejection> {
    let token = &get_secret("channel.token");
    let message = convert_stories_to_message().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let url = get_config("message.broadcast_url");

    let json_body = match serde_json::to_string(&request_body) {
        Ok(body) => body,
        Err(e) => {
            log::error!("Failed to serialize broadcast request: {}", e);
            return Err(warp::reject::custom(AppError::JsonParse(e)));
        }
    };

    request_handler::handle_send_request(token, json_body, url.as_str()).await
}

pub async fn broadcast_daily_summary() -> Result<impl Reply, Rejection> {
    let token = get_secret("channel.token");

    let url = get_config("message.broadcast_url");

    let message = get_chatgpt_summary().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let json_body = match serde_json::to_string(&request_body) {
        Ok(body) => body,
        Err(e) => {
            log::error!("Failed to serialize daily summary request: {}", e);
            return Err(warp::reject::custom(AppError::JsonParse(e)));
        }
    };

    request_handler::handle_send_request(token.as_str(), json_body, url.as_str()).await
}

async fn reply_latest_story(token: &str, reply_token: &str) -> Result<impl Reply, Rejection> {
    let message = convert_stories_to_message().await;

    let request_body = LineMessageRequest {
        replyToken: reply_token.to_string(),
        messages: vec![message],
    };

    let json_body = match serde_json::to_string(&request_body) {
        Ok(body) => body,
        Err(e) => {
            log::error!("Failed to serialize reply request: {}", e);
            return Err(warp::reject::custom(AppError::JsonParse(e)));
        }
    };

    let url = config_helper::get_config("message.reply_url");

    request_handler::handle_send_request(token, json_body, url.as_str()).await
}

async fn push_summary(
    token: &str,
    user_id: &str,
    language_code: String,
    indexes: Vec<usize>,
) -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;

    let mut messages = Vec::new();

    for index in indexes {
        let story = &stories[index - 1];
        let story_summary = match kagi::get_kagi_summary(story.storylink.to_owned()).await {
            Ok(summary) => summary,
            Err(e) => {
                log::error!("Failed to get Kagi summary for index {}: {}", index, e);
                "Summary unavailable".to_string()
            }
        };
        let summary_zhtw = match chatgpt::translate(story_summary.clone(), language_code.to_owned()).await {
            Ok(translated) => translated,
            Err(e) => {
                log::error!("Failed to translate summary for index {}: {}", index, e);
                story_summary // Use original if translation fails
            }
        };
        messages.push(summary_zhtw);
    }

    let result = push_messages(token, user_id, messages).await;
    result
}

async fn push_url_summary(
    token: &str,
    user_id: &str,
    language_code: String,
    url: String,
) -> Result<impl Reply, Rejection> {

    let story_summary = match kagi::get_kagi_summary(url.to_owned()).await {
        Ok(summary) => summary,
        Err(e) => {
            log::error!("Failed to get Kagi summary for URL: {}", e);
            "Summary unavailable".to_string()
        }
    };
    let summary_zhtw = match chatgpt::translate(story_summary.clone(), language_code.to_owned()).await {
        Ok(translated) => translated,
        Err(e) => {
            log::error!("Failed to translate URL summary: {}", e);
            story_summary // Use original if translation fails
        }
    };
    let messages = vec![summary_zhtw];

    let result = push_messages(token, user_id, messages).await;
    result
}

async fn push_messages(
    token: &str,
    user_id: &str,
    text: Vec<String>,
) -> Result<impl Reply + Sized + Sized, Rejection> {
    let messages: Vec<LineMessage> = text
        .iter()
        .map(|t| LineMessage {
            message_type: "text".to_string(),
            text: t.to_string(),
        })
        .collect();

    let request = LineSendMessageRequest {
        to: user_id.to_string(),
        messages,
    };

    let json_body = match serde_json::to_string(&request) {
        Ok(body) => body,
        Err(e) => {
            log::error!("Failed to serialize push message request: {}", e);
            return Err(warp::reject::custom(AppError::JsonParse(e)));
        }
    };

    log::info!("{}", &json_body);

    let url = get_config("message.push_url");

    request_handler::handle_send_request(token, json_body, url.as_str()).await
}


async fn convert_stories_to_message() -> LineMessage {
    let message_text = combine_stories().await;

    let message = convert_to_line_message(message_text).await;
    message
}

async fn combine_stories() -> String {
    let stories = readrss::get_last_hn_stories().await;
    let message_text = stories
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {} ({})", i + 1, s.story.clone(), s.storylink))
        .collect::<Vec<String>>()
        .join("\n\n");
    message_text
}

async fn get_chatgpt_summary() -> LineMessage {
    let stories = combine_stories().await;
    let summary = match chatgpt::get_chatgpt_summary(stories).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to get ChatGPT summary: {}", e);
            "Summary unavailable".to_string()
        }
    };

    log::info!("summary message: {}", summary);

    let message = convert_to_line_message(summary).await;
    message
}

async fn convert_to_line_message(summary: String) -> LineMessage {
    let message = LineMessage {
        message_type: "text".to_string(),
        text: summary,
    };
    message
}
