use bytes::Bytes;
use serde_json::{json, Value};
use warp::{
    http::{Response, StatusCode},
    Rejection, Reply,
};
use warp::hyper::Body;

use crate::{chatgpt, config_helper, kagi, line_helper, readrss, request_handler};
use crate::config_helper::{get_config, get_secret};
use crate::line_helper::{
    LineBroadcastRequest, LineMessage, LineMessageRequest, LineSendMessageRequest,
};

pub async fn conversation_handler(content: Bytes) -> Result<impl Reply, Rejection> {
    let conversions = String::from_utf8(content.to_vec()).unwrap();
    let res = chatgpt::run_conversation(conversions).await;

    let function_call: Value = serde_json::from_str(res.as_ref().unwrap().as_str()).unwrap();

    log::info!("function_call: {}", function_call);

    match function_call.get("name").and_then(Value::as_str) {
        Some(function_name) => {
            let arguments_value = function_call["arguments"].as_str().unwrap();
            let arguments: Value = serde_json::from_str(arguments_value).unwrap();

            log::info!("arguments: {}", arguments);

            if function_name == "push_summary" {
                let index = arguments["indexes"].as_array().unwrap();
                log::info!("index: {:?}", index); // Convert Vec<usize> to string representation
            }

            let response = warp::reply::json(&json!(function_call));
            Ok(warp::reply::with_status(response, StatusCode::OK))
        }
        None => {
            let response = warp::reply::json(&json!({
                "message": function_call["message"].as_str().unwrap(),
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

    // Clone or copy necessary data for the new task
    let body_clone = body.clone();

    // Process the other logic asynchronously
    tokio::spawn(async move {
        process_request(body_clone).await;
    });

    match validation_result {
        Ok(()) => {
            // Immediately return HTTP 200 OK after signature validation
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": true})),
                StatusCode::OK,
            ))
        },
        Err(_e) => {
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
    let json_value: Value = serde_json::from_slice(&body).unwrap();

    // Extract the text from the first message
    let text = json_value["events"]
        .get(0)
        .and_then(|event| event["message"].get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default()
        .to_string();

    let language_code = chatgpt::get_language_code(text.to_owned()).await.unwrap();

    let reply_token = json_value["events"][0]["replyToken"].as_str();

    let user_id = json_value["events"][0]["source"]["userId"].as_str();

    let res = chatgpt::run_conversation(text).await.unwrap();

    let function_call: Value = serde_json::from_str(res.as_str()).unwrap();

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
            handle_reply_latest_story(&channel_token, &reply_token.unwrap().to_string()).await;
        }
        Some("push_summary") => {
            handle_push_summary(&channel_token, &user_id.unwrap(), language_code, &function_call).await;
        }
        Some("push_url_summary") => {
            handle_push_url_summary(&channel_token, &user_id.unwrap(), "zh-tw".to_string(), &function_call).await;
        }
        _ => {
            handle_push_messages(&channel_token, &user_id.unwrap(), &function_call).await;
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
    let arguments: Value = serde_json::from_str(function_call["arguments"].as_str().unwrap()).unwrap();
    let indexes = arguments
        .get("indexes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|i| i.as_u64().unwrap() as usize)
        .collect::<Vec<usize>>();

    match push_summary(channel_token, user_id, language_code, indexes).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error push summary").await;
        }
    }
}

async fn handle_push_messages(channel_token: &str, user_id: &str, function_call: &Value) {
    match push_messages(
        channel_token,
        user_id,
        vec![function_call["message"].as_str().unwrap().to_string()],
    ).await {
        Ok(_) => {},
        Err(_e) => {
            handle_error_response("Error push messages").await;
        }
    }
}

async fn handle_push_url_summary(channel_token: &str, user_id: &str, language_code: String, function_call: &Value) {
    let arguments = function_call.get("arguments").unwrap().as_str().unwrap();
    let arguments_json: Value = serde_json::from_str(arguments).unwrap();
    let url = arguments_json.get("url").unwrap().as_str().unwrap().to_string();
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
    let channel = readrss::read_feed()
        .await
        .map_err(|_| reply_error_msg("Error fetching feed", StatusCode::INTERNAL_SERVER_ERROR))
        .unwrap();

    let latest_item = readrss::get_latest_item(&channel)
        .ok_or_else(|| reply_error_msg("No items in feed", StatusCode::NOT_FOUND))
        .unwrap();

    let latest_title = latest_item.title().unwrap_or("Untitled item").to_string();

    let response = Response::builder()
        .header("content-type", "text/plain")
        .status(StatusCode::OK)
        .body(Bytes::from(latest_title))
        .unwrap();

    Ok(response)
}

fn reply_error_msg(error: &'static str, status: StatusCode) -> Response<Bytes> {
    let error_msg = Bytes::from(error);
    Response::builder()
        .header("content-type", "text/plain")
        .status(status)
        .body(error_msg)
        .unwrap()
}

pub async fn send_line_broadcast() -> Result<impl Reply, Rejection> {
    let token = &get_secret("channel.token");
    let message = convert_stories_to_message().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let url = get_config("message.broadcast_url");

    let json_body = serde_json::to_string(&request_body).unwrap();

    request_handler::handle_send_request(token, json_body, url.as_str()).await
}

pub async fn broadcast_daily_summary() -> Result<impl Reply, Rejection> {
    let token = get_secret("channel.token");

    let url = get_config("message.broadcast_url");

    let message = get_chatgpt_summary().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let json_body = serde_json::to_string(&request_body).unwrap();

    request_handler::handle_send_request(token.as_str(), json_body, url.as_str()).await
}

async fn reply_latest_story(token: &str, reply_token: &str) -> Result<impl Reply, Rejection> {
    let message = convert_stories_to_message().await;

    let request_body = LineMessageRequest {
        replyToken: reply_token.to_string(),
        messages: vec![message],
    };

    let json_body = serde_json::to_string(&request_body).unwrap();

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
        let story_summary = kagi::get_kagi_summary(story.storylink.to_owned()).await;
        let summary_zhtw = chatgpt::translate(story_summary, language_code.to_owned())
            .await
            .unwrap();
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

    let story_summary = kagi::get_kagi_summary(url.to_owned()).await;
    let summary_zhtw = chatgpt::translate(story_summary, language_code.to_owned())
            .await
            .unwrap();
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

    let json_body = serde_json::to_string(&request).unwrap();

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
    let summary = chatgpt::get_chatgpt_summary(stories).await.unwrap();

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
