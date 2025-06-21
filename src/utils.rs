use once_cell::sync::Lazy;
use std::time::Duration;
use tokio_retry::{strategy::ExponentialBackoff, Retry};
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use warp::reject::Rejection;
use warp::reply::Reply;

use crate::models::{ApiError, ApiResult, ErrorResponse, SuccessResponse};

/// Shared HTTP client instance for reuse across the application
pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to create HTTP client")
});

/// Configuration cache to avoid repeated config lookups
#[derive(Debug, Clone)]
pub struct ConfigCache {
    // URLs
    pub chat_completions_url: String,
    pub kagi_summarize_url: String,
    pub rss_feed_url: String,
    pub line_broadcast_url: String,
    pub line_reply_url: String,
    pub line_push_url: String,
    
    // Models and settings
    pub chatgpt_model: String,
    pub chatgpt_translate_model: String,
    pub kagi_engine: String,
    pub kagi_target_language: String,
    
    // Secrets
    pub channel_secret: String,
    pub channel_token: String,
    pub chatgpt_secret: String,
    pub kagi_secret: String,
}

impl ConfigCache {
    pub fn new() -> Self {
        use crate::config;
        
        Self {
            // URLs
            chat_completions_url: config::get_config("chatgpt.chat_completions_url"),
            kagi_summarize_url: config::get_config("kagi.kagi_summarize_url"),
            rss_feed_url: config::get_config("rss.feed_url"),
            line_broadcast_url: config::get_config("message.broadcast_url"),
            line_reply_url: config::get_config("message.reply_url"),
            line_push_url: config::get_config("message.push_url"),
            
            // Models and settings
            chatgpt_model: config::get_config("chatgpt.model"),
            chatgpt_translate_model: config::get_config("chatgpt.translate_model"),
            kagi_engine: config::get_config("kagi.engine"),
            kagi_target_language: config::get_config("kagi.target_language"),
            
            // Secrets
            channel_secret: config::get_secret("channel.secret"),
            channel_token: config::get_secret("channel.token"),
            chatgpt_secret: config::get_secret("chatgpt.secret"),
            kagi_secret: config::get_secret("kagi.secret"),
        }
    }
}

/// Global config cache instance
pub static CONFIG_CACHE: Lazy<ConfigCache> = Lazy::new(ConfigCache::new);

/// Retry strategy for external API calls
pub fn create_retry_strategy() -> std::iter::Take<ExponentialBackoff> {
    ExponentialBackoff::from_millis(100)
        .max_delay(Duration::from_secs(5))
        .take(3) // 3 retries maximum
}

/// Execute an async function with retry logic for network operations
pub async fn with_retry<T, F, Fut>(operation: F) -> ApiResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ApiResult<T>>,
{
    let retry_strategy = create_retry_strategy();
    
    let result = Retry::spawn(retry_strategy, || async {
        operation().await.map_err(|e| match e {
            ApiError::NetworkError(_) => "retryable",
            _ => "non-retryable",
        })
    })
    .await;

    match result {
        Ok(value) => Ok(value),
        Err(_) => operation().await, // Return the actual error from the last attempt
    }
}

/// Create a successful JSON response
pub fn json_success() -> Response<Body> {
    let success = SuccessResponse { success: true };
    let json = serde_json::to_string(&success).unwrap();
    
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(json))
        .unwrap()
}

/// Create an error JSON response
pub fn json_error(error: &str, status: StatusCode) -> Response<Body> {
    let error_response = ErrorResponse::new(error);
    let json = serde_json::to_string(&error_response).unwrap();
    
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(json))
        .unwrap()
}

/// Handle errors from rejections
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    log::error!("Request error: {:?}", err);
    
    let error_msg = "Internal server error";
    let code = StatusCode::INTERNAL_SERVER_ERROR;
    
    let json = warp::reply::json(&ErrorResponse::new(error_msg));
    Ok(warp::reply::with_status(json, code))
}

/// Generate a random string for testing
#[cfg(test)]
pub fn random_string(length: usize) -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
} 