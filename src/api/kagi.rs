use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{ApiError, ApiResult};
use crate::utils::{HTTP_CLIENT, CONFIG_CACHE};

/// Request for Kagi summarization
#[derive(Debug, Serialize)]
struct KagiSummarizeRequest {
    url: String,
    engine: String,
    target_language: String,
}

/// Response from Kagi summarization
#[derive(Debug, Deserialize)]
struct KagiSummarizeResponse {
    meta: KagiResponseMeta,
    data: Option<KagiResponseData>,
    error: Option<String>,
}

/// Metadata from Kagi response
#[derive(Debug, Deserialize)]
struct KagiResponseMeta {
    id: String,
    node: Option<String>,
    timing: f64,
    info: Option<String>,
}

/// Data from Kagi response
#[derive(Debug, Deserialize)]
struct KagiResponseData {
    summary: String,
    tokens: Option<i32>,
}

/// Create Kagi API headers
fn create_kagi_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    
    // Use try_from instead of from_str to handle invalid header value characters safely
    match HeaderValue::try_from(format!("Bot {}", token)) {
        Ok(auth_value) => {
            headers.insert("Authorization", auth_value);
        },
        Err(e) => {
            log::error!("Failed to create Authorization header: {}", e);
            // Continue with empty Authorization, which will fail at the API level
            // but won't cause a panic in our application
        }
    }
    
    headers
}

/// Summarize content from a URL
pub async fn summarize_url(url: &str) -> ApiResult<String> {
    // Get configuration from cache
    let api_key = &CONFIG_CACHE.kagi_secret;
    let api_url = &CONFIG_CACHE.kagi_summarize_url;
    let engine = &CONFIG_CACHE.kagi_engine;
    let target_language = &CONFIG_CACHE.kagi_target_language;
    
    // Create request body
    let request_body = json!({
        "url": url,
        "engine": engine,
        "target_language": target_language
    });
    
    // Send the request
    let response = HTTP_CLIENT
        .post(api_url)
        .headers(create_kagi_headers(api_key))
        .json(&request_body)
        .send()
        .await?;
    
    // Parse the response
    if !response.status().is_success() {
        return Err(ApiError::ExternalServiceError(format!(
            "Kagi API error: HTTP {}", 
            response.status()
        )));
    }
    
    let response_body = response.text().await?;
    let kagi_response: KagiSummarizeResponse = serde_json::from_str(&response_body)?;
    
    // Check for errors in the response
    if let Some(error) = kagi_response.error {
        return Err(ApiError::ExternalServiceError(format!("Kagi API error: {}", error)));
    }
    
    // Extract the summary
    if let Some(data) = kagi_response.data {
        Ok(data.summary)
    } else {
        Err(ApiError::ExternalServiceError("No summary data returned from Kagi".to_string()))
    }
} 