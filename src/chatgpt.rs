use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
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

pub async fn get_chatgpt_summary(tldr_page_url: String) -> String {
    let api_secret = crate::r#mod::get_config("chatgpt.secret");

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", api_secret).parse().unwrap(),
    );

    let url = crate::r#mod::get_config("chatgpt.chat_completions_url");

    let model = crate::r#mod::get_config("chatgpt.model");

    let _content = String::from("tldr ") + tldr_page_url.as_str();

    let message = ChatMessage {
        role: "user".to_owned(),
        content: _content,
    };

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![message],
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
    let clean_res_content = res_content.replace("\n", "");

    return clean_res_content;
}
