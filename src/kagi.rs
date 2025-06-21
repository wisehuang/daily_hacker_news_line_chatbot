use crate::config_helper::{get_config, get_secret};
use crate::errors::AppError;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct KagiSummaryRequest {
    url: String,
    engine: String,
    target_language: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Meta {
    id: String,
    node: String,
    ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Data {
    output: String,
    tokens: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct KagiSummaryResponse {
    meta: Meta,
    data: Data,
}

pub async fn get_kagi_summary(tldr_page_url: String) -> Result<String, AppError> {
    let api_token = get_secret("kagi.token");

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse()
        .map_err(|e| AppError::Config(format!("Invalid content type header: {}", e)))?);
    headers.insert(AUTHORIZATION, format!("Bot {}", api_token).parse()
        .map_err(|e| AppError::Config(format!("Invalid authorization header: {}", e)))?);

    let url = get_config("kagi.kagi_summarize_url");

    let engine = get_config("kagi.engine");

    let target_language = get_config("kagi.target_language");

    let request = KagiSummaryRequest {
        url: tldr_page_url,
        engine,
        target_language,
    };

    let json_body = serde_json::to_string(&request)
        .map_err(|e| AppError::JsonParse(e))?;

    log::info!("Kagi summary API request: {}", json_body);

    let response = client
        .post(url)
        .headers(headers)
        .body(json_body)
        .send()
        .await
        .map_err(|e| AppError::Network(e))?;

    let response_text = response.text().await
        .map_err(|e| AppError::Network(e))?;

    log::info!("Kagi summary API response: {}", response_text);

    let response_struct: Result<KagiSummaryResponse, serde_json::Error> = serde_json::from_str(&response_text);

    match response_struct {
        Ok(response) => {
            let res_content = response.data.output.clone();
            Ok(res_content.replace("\n", ""))
        },
        Err(e) => {
            log::error!("Failed to parse Kagi response: {}", e);
            log::debug!("Raw Kagi response: {}", response_text);
            Err(AppError::JsonParse(e))
        }
    }
}