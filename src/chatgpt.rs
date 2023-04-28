use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
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

pub async fn get_chatgpt_summary(stories: String) -> String {
    let api_secret = get_config("chatgpt.secret");

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", api_secret).parse().unwrap(),
    );

    let url = get_config("chatgpt.chat_completions_url");

    let model = get_config("chatgpt.model");

    let _content = String::from("這是今天 daily hacker news 的 top 10, 幫我融會貫通, 重點整理:並且找出最重要的新聞以及相關的重要關鍵字, 並且適當的分段, 分段符號使用('\n\n'): ") + stories.as_str();

    let message = ChatMessage {
        role: "user".to_owned(),
        content: _content,
    };

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![message],
        temperature: 0.05,
        max_tokens: 2048,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
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
    let response_struct: ChatCompletion = serde_json::from_str(&response_text).unwrap();

    let res_content = response_struct.choices[0].message.content.clone();
    // let clean_res_content = res_content.replace("\n", "");

    return res_content;
}