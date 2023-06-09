use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use crate::config_helper::get_config;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: usize,
    top_p: f64,
    frequency_penalty: f64,
    presence_penalty: f64,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    id: String,
    object: String,
    created: i64,
    model: String,
    usage: Usage,
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
    finish_reason: String,
    index: i32,
}

#[derive(Debug, Deserialize)]
struct Message {
    role: String,
    content: String,
}

pub async fn get_chatgpt_summary(stories: String) -> Result<String, Box<dyn std::error::Error>> {
    let api_secret = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config("chatgpt.model");

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: format!(
                "這是今天 daily hacker news 的 top 10, 幫我融會貫通, 重點整理:並且找出最重要的新聞以及相關的重要關鍵字, 並且適當的分段, 分段符號使用('\n\n'): {}",
                stories
            ),
        }],
        temperature: 0.05,
        max_tokens: 2048,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
    };
    let res_content = send_chat_request(api_secret, url, request).await?;
    Ok(res_content)
}

pub async fn translate_to_zhtw(content: String) -> Result<String, Box<dyn std::error::Error>> {
    let api_secret = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config("chatgpt.model");

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: format!("翻譯成繁體中文: {}", content),
        }],
        temperature: 0.05,
        max_tokens: 2048,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
    };
    let res_content = send_chat_request(api_secret, url, request).await?;
    Ok(res_content)
}

async fn send_chat_request(api_secret: String, url: String, request: ChatRequest) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let json_body = serde_json::to_string(&request)?;

    let response = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_secret))
        .body(json_body)
        .send()
        .await?;
    let response_text = response.text().await?;
    let response_struct: ChatCompletion = serde_json::from_str(&response_text)?;

    let res_content = response_struct.choices[0].message.content.clone();
    Ok(res_content)
}