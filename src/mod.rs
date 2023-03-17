use bytes::Bytes;
use serde_json::{json, Value};

use crate::{
    chatgpt,
    line_helper,
    readrss,
    config_helper,
    request_handler,
};

use crate::line_helper::{LineMessage, LineBroadcastRequest, LineMessageRequest};

use warp::{
    http::{Response, StatusCode},
    Rejection, Reply,
};

pub async fn parse_request_handler(
    x_line_signature: String,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    match line_helper::is_signature_valid(x_line_signature, &body) {
        Ok(_) => {}
        Err(e) => {
            let error_msg = json!({"success": false, "error": e.to_string()});
            let response = warp::reply::with_status(
                warp::reply::json(&error_msg),
                warp::http::StatusCode::BAD_REQUEST,
            );
            return Ok(response);
        }
    }

    let channel_token = config_helper::get_config("channel.token");

    // Parse the body as a LineWebhookRequest
    let json_value: Value = serde_json::from_slice(&body).unwrap();

    // Extract the text from the first message
    let text = json_value["events"]
        .get(0)
        .and_then(|event| event["message"].get("text"))
        .and_then(|text| text.as_str());

    let reply_token = json_value["events"][0]["replyToken"].as_str();

    if "today" == text.unwrap() {
        reply_latest_story(&channel_token, &reply_token.unwrap().to_string()).await?;
    }

    if let Ok(index) = text.unwrap().parse::<usize>() {
        if index < 1 || index > 10 {
            reply_error(
                &channel_token,
                &reply_token.unwrap().to_string(),
                "Incorrect number",
            )
            .await?;
        }

        match reply_tldr(&channel_token, &reply_token.unwrap().to_string(), index).await {
            Ok(_) => {}
            Err(_) => {
                reply_error(
                    &channel_token,
                    &reply_token.unwrap().to_string(),
                    "Something wrong, please try again",
                )
                .await?;
            }
        }
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&json!({"success": true})),
        warp::http::StatusCode::OK,
    ))
}


pub async fn get_latest_stories() -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    Ok(warp::reply::json(&stories))
}

pub async fn get_latest_title() -> Result<impl Reply, Rejection> {
    match readrss::read_feed().await {
        Ok(channel) => {
            if let Some(latest_item) = readrss::get_latest_item(&channel) {
                let latest_title = latest_item.title().unwrap_or("Untitled item").to_string();
                let response = Response::builder()
                    .header("content-type", "text/plain")
                    .status(StatusCode::OK)
                    .body(Bytes::from(latest_title))
                    .unwrap();
                Ok(response)
            } else {
                let response = Response::builder()
                    .header("content-type", "text/plain")
                    .status(StatusCode::NOT_FOUND)
                    .body(Bytes::from("No items in feed"))
                    .unwrap();
                Ok(response)
            }
        }
        Err(_) => {
            let response = Response::builder()
                .header("content-type", "text/plain")
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Bytes::from("Error fetching feed"))
                .unwrap();
            Ok(response)
        }
    }
}

pub async fn send_line_broadcast() -> Result<impl Reply, Rejection> {
    let token = &config_helper::get_config("channel.token");
    let message = convert_stories_to_message().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let url = config_helper::get_config("message.broadcast_url");

    let json_body = serde_json::to_string(&request_body).unwrap();

    request_handler::handle_send_request(token, json_body, url.as_str()).await
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

async fn reply_tldr(token: &str, reply_token: &str, index: usize) -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    let story = &stories[index - 1];

    let story_summary = chatgpt::get_chatgpt_summary(story.storylink.to_owned()).await;

    reply_message(token, reply_token, story_summary.as_str()).await
}

async fn reply_error(
    token: &str,
    reply_token: &str,
    error_msg: &str,
) -> Result<impl Reply, Rejection> {
    reply_message(token, reply_token, error_msg).await
}

async fn reply_message(
    token: &str,
    reply_token: &str,
    text: &str,
) -> Result<impl Reply + Sized + Sized, Rejection> {
    let message = LineMessage {
        message_type: "text".to_string(),
        text: text.to_string(),
    };

    let request_body = LineMessageRequest {
        replyToken: reply_token.to_string(),
        messages: vec![message],
    };

    let json_body = serde_json::to_string(&request_body).unwrap();

    log::info!("{}", &json_body);

    let url = config_helper::get_config("message.reply_url");

    request_handler::handle_send_request(token, json_body, url.as_str()).await
}

async fn convert_stories_to_message() -> LineMessage {
    let stories = readrss::get_last_hn_stories().await;
    let message_text = stories
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {} ({})", i + 1, s.story.clone(), s.storylink))
        .collect::<Vec<String>>()
        .join("\n\n");

    let message = LineMessage {
        message_type: "text".to_string(),
        text: message_text,
    };
    message
}
