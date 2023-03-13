use std::error::Error;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::Bytes;
use config::{Config, File, FileFormat};
use hmac::{Hmac, Mac};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;

use warp::{
    http::{Response, StatusCode},
    Rejection, Reply,
};
use crate::{chatgpt, readrss};

#[derive(Serialize, Deserialize)]
struct LineMessage {
    #[serde(rename = "type")]
    message_type: String,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct LineBroadcastRequest {
    messages: Vec<LineMessage>,
}

#[derive(Serialize, Deserialize)]
struct LineMessageRequest {
    replyToken: String,
    messages: Vec<LineMessage>,
}

#[derive(Deserialize, Debug)]
struct LineErrorResponse {
    message: String,
    details: Vec<LineErrorDetail>,
}

#[derive(Deserialize, Debug)]
struct LineErrorDetail {
    message: String,
    property: String,
}

pub fn get_config(config_name: &str) -> String {
    let config_builder = Config::builder().add_source(File::new("config.toml", FileFormat::Toml));

    let config_value: String = match config_builder.build() {
        Ok(config) => config
            .get::<String>(config_name)
            .expect("Missing config_name in config file"),
        Err(e) => {
            panic!("{}", e);
        }
    };
    config_value
}

pub async fn parse_request_handler(
    x_line_signature: String,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    match is_signature_valid(x_line_signature, &body) {
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

    let channel_token = get_config("channel.token");

    // Parse the body as a LineWebhookRequest
    let json_value: Value = serde_json::from_slice(&body).unwrap();

    // Extract the text from the first message
    let text = json_value["events"]
        .get(0)
        .and_then(|event| event["message"].get("text"))
        .and_then(|text| text.as_str());

    let reply_token = json_value["events"][0]["replyToken"].as_str();

    if "today" == text.unwrap() {        
        reply_latest_story(&channel_token, &reply_token.unwrap().to_string()).await;
    }

    if let Ok(index) = text.unwrap().parse::<usize>() {
        match reply_tldr(&channel_token, &reply_token.unwrap().to_string(), index).await {
            Ok(_) => {}
            Err(_) => {
                reply_error(&channel_token, &reply_token.unwrap().to_string()).await;
            }
        }
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&json!({"success": true})),
        warp::http::StatusCode::OK,
    ))
}

fn is_signature_valid(x_line_signature: String, body: &Bytes) -> Result<(), Box<dyn Error>> {
    let channel_secret = get_config("channel.secret");

    log::info!("channel secret: {}", channel_secret);

    let encoded_body = generate_signature(&channel_secret, &body);

    log::info!("encoded body: {}", encoded_body);
    log::info!("x-line-signature: {:?}", x_line_signature);
    log::info!(
        "body content: {}",
        String::from_utf8(body.to_vec()).unwrap()
    );

    if encoded_body != x_line_signature {
        return Err("Invalid signature".into());
    }

    Ok(())
}

pub async fn get_latest_stories() -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    Ok(warp::reply::json(&stories))
}

fn generate_signature(channel_secret: &str, body: &[u8]) -> String {
    let mut hmac_sha256 =
        Hmac::<Sha256>::new_from_slice(channel_secret.as_bytes()).expect("Failed to create HMAC");
    hmac_sha256.update(&body);

    BASE64.encode(hmac_sha256.finalize().into_bytes())
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
    let token = &get_config("channel.token");
    let message = convert_stories_to_message().await;

    let request_body = LineBroadcastRequest {
        messages: vec![message],
    };

    let url = get_config("message.broadcast_url");

    let json_body = serde_json::to_string(&request_body).unwrap();

    handle_send_request(token, json_body, url.as_str()).await
    
}

async fn reply_latest_story(token: &str, reply_token: &str) -> Result<impl Reply, Rejection> {
    let message = convert_stories_to_message().await;

    let request_body = LineMessageRequest {
        replyToken: reply_token.to_string(),
        messages: vec![message],
    };

    let json_body = serde_json::to_string(&request_body).unwrap();

    let url = get_config("message.reply_url");

    handle_send_request(token, json_body, url.as_str()).await
}

async fn handle_send_request(token: &str, json_body: String, url: &str) -> Result<impl Reply + Sized, Rejection> {
    match send_request(token, json_body, url).await {
        Ok(_response) => {
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": true})),
                warp::http::StatusCode::OK,
            ))
        }
        Err(_error) => {
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": false, "error": _error.to_string()})),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

async fn reply_tldr(token: &str, reply_token: &str, index: usize) -> Result<impl Reply, Rejection> {
    let stories = readrss::get_last_hn_stories().await;
    let story = &stories[index - 1];

    let story_summary = chatgpt::get_chatgpt_summary(story.storylink.to_owned()).await;

    reply_message(token, reply_token, story_summary.as_str()).await
}

async fn reply_error (token: &str, reply_token: &str) -> Result<impl Reply, Rejection> {
    reply_message(token, reply_token, "Something wrong, please try again").await
}

async fn reply_message(token: &str, reply_token: &str, text: &str) -> Result<impl Reply + Sized + Sized, Rejection> {
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

    let url = get_config("message.reply_url");

    handle_send_request(token, json_body, url.as_str()).await
}

async fn send_request(token: &str, json_body: String, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());

    let response = client
        .post(url)
        .headers(headers)
        .body(json_body)
        .send()
        .await
        .unwrap();

    return Ok(response);
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
