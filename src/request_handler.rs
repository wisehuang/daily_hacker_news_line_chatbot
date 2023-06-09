use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde_json::json;
use warp::{
    Rejection, Reply,
};
use uuid::Uuid;

pub async fn handle_send_request(
    token: &str,
    json_body: String,
    url: &str,
) -> Result<impl Reply + Sized, Rejection> {
    match send_request(token, json_body, url).await {
        Ok(_response) => {            
            log::info!("LINE Message API response: {}", _response.text().await.unwrap());
            
            Ok(warp::reply::with_status(
            warp::reply::json(&json!({"success": true})),
            warp::http::StatusCode::OK,
        ))},
        Err(_error) => {
            log::error!("LINE Message API error: {}", _error.to_string());
            Ok(warp::reply::with_status(
            warp::reply::json(&json!({"success": false, "error": _error.to_string()})),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))},
    }
}

pub async fn send_request(
    token: &str,
    json_body: String,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    headers.insert("X-Line-Retry-Key", Uuid::new_v4().to_string().parse().unwrap());

    let response = client
        .post(url)
        .headers(headers)
        .body(json_body)
        .send()
        .await?;

    Ok(response)
}