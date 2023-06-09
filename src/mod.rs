use bytes::Bytes;
use serde_json::{json, Value};

use crate::{chatgpt, config_helper, kagi, line_helper, readrss, request_handler};

use crate::line_helper::{
    LineBroadcastRequest, LineMessage, LineMessageRequest, LineSendMessageRequest,
};

use warp::{
    http::{Response, StatusCode},
    Rejection, Reply,
};

pub async fn parse_request_handler(
    x_line_signature: String,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    // Check if the signature is valid
    line_helper::is_signature_valid(x_line_signature, &body)
        .map_err(|e| {
            let error_msg = json!({"success": false, "error": e.to_string()});
            warp::reply::with_status(
                warp::reply::json(&error_msg),
                warp::http::StatusCode::BAD_REQUEST,
            )
            .into_response()
        })
        .unwrap();

    // Get the channel token from the configuration file
    let channel_token = config_helper::get_config("channel.token");

    // Parse the body as a LineWebhookRequest
    let json_value: Value = serde_json::from_slice(&body)
        .map_err(|e| {
            let error_msg = json!({"success": false, "error": e.to_string()});
            warp::reply::with_status(
                warp::reply::json(&error_msg),
                warp::http::StatusCode::BAD_REQUEST,
            )
            .into_response()
        })
        .unwrap();

    // Extract the text from the first message
    let text = json_value["events"]
        .get(0)
        .and_then(|event| event["message"].get("text"))
        .and_then(|text| text.as_str());

    let reply_token = json_value["events"][0]["replyToken"].as_str();

    let user_id = json_value["events"][0]["source"]["userId"].as_str();

    // Handle "today" command
    if let Some("today") = text {
        reply_latest_story(&channel_token, &reply_token.unwrap().to_string())
            .await
            .map_err(|e| {
                let error_msg = json!({"success": false, "error": "Error sending message"});
                warp::reply::with_status(
                    warp::reply::json(&error_msg),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response()
            })
            .unwrap();
    }

    // Handle index command
    if let Some(index_str) = text {
        if let Ok(index) = index_str.parse::<usize>() {
            if index < 1 || index > 10 {
                reply_error(
                    &channel_token,
                    &reply_token.unwrap().to_string(),
                    "Incorrect number",
                )
                .await
                .map_err(|e| {
                    let error_msg = json!({"success": false, "error": "Error reply message"});
                    warp::reply::with_status(
                        warp::reply::json(&error_msg),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response()
                })
                .unwrap();
            } else {
                push_summary(&channel_token, &user_id.unwrap(), index)
                    .await
                    .map_err(|_e| {
                        let error_msg = json!({"success": false, "error": "Error push summary"});
                        warp::reply::with_status(
                            warp::reply::json(&error_msg),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        )
                        .into_response()
                    })
                    .unwrap();
            }
        }
    }

    // Return success response
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

pub async fn broadcast_daily_summary() -> Result<impl Reply, Rejection> {
    let token = &config_helper::get_config("channel.token");

    let url = config_helper::get_config("message.broadcast_url");

    let message = get_chatgpt_summary().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

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

async fn push_summary(token: &str, user_id: &str, index: usize) -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    let story = &stories[index - 1];

    let story_summary = kagi::get_kagi_summary(story.storylink.to_owned()).await;

    let summary_zhtw = chatgpt::translate_to_zhtw(story_summary).await.unwrap();

    let result = push_message(token, user_id, summary_zhtw.as_str()).await;
    result
}

async fn reply_error(
    token: &str,
    reply_token: &str,
    error_msg: &str,
) -> Result<impl Reply, Rejection> {
    reply_message(token, reply_token, error_msg).await
}

async fn push_message(
    token: &str,
    user_id: &str,
    text: &str,
) -> Result<impl Reply + Sized + Sized, Rejection> {
    let request = LineSendMessageRequest {
        to: user_id.to_string(),
        messages: vec![LineMessage {
            message_type: "text".to_string(),
            text: text.to_string(),
        }],
    };

    let json_body = serde_json::to_string(&request).unwrap();

    log::info!("{}", &json_body);

    let url = config_helper::get_config("message.push_url");

    request_handler::handle_send_request(token, json_body, url.as_str()).await

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

    let request = LineMessageRequest {
        replyToken: reply_token.to_string(),
        messages: vec![message],
    };

    let json_body = serde_json::to_string(&request).unwrap();

    log::info!("{}", &json_body);

    let url = config_helper::get_config("message.reply_url");

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
