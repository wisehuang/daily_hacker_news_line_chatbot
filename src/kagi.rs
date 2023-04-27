use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use crate::config_helper::get_config;

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

pub async fn get_kagi_summary(tldr_page_url: String) -> String {
    let api_token = get_config("kagi.token");

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(
        AUTHORIZATION,
        format!("Bot {}", api_token).parse().unwrap(),
    );

    let url = get_config("kagi.kagi_summarize_url");

    let engine = get_config("kagi.engine");

    let target_language = get_config("kagi.target_language");

    let _content = String::from(tldr_page_url.as_str());

    let request = KagiSummaryRequest {
        url: _content,
        engine: engine,
        target_language: target_language,
    };

    let json_body = serde_json::to_string(&request).unwrap();

    let response = client
        .post(url)
        .headers(headers)
        .body(json_body)
        .send()
        .await
        .unwrap();

    let response_text = response.text().await.unwrap();
    let response_struct: KagiSummaryResponse = serde_json::from_str(&response_text).unwrap();

    let res_content = response_struct.data.output.clone();
    let clean_res_content = res_content.replace("\n", "");

    return clean_res_content;
}