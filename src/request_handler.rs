use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde_json::json;
use warp::{
    Rejection, Reply,
};

pub async fn handle_send_request(
    token: &str,
    json_body: String,
    url: &str,
) -> Result<impl Reply + Sized, Rejection> {
    match send_request(token, json_body, url).await {
        Ok(_response) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"success": true})),
            warp::http::StatusCode::OK,
        )),
        Err(_error) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"success": false, "error": _error.to_string()})),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
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

    let response = client
        .post(url)
        .headers(headers)
        .body(json_body)
        .send()
        .await
        .unwrap();

    return Ok(response);
}