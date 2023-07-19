use crate::config_helper::get_config;
use crate::json;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

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

pub async fn run_conversation(content: String) -> Result<String, Box<dyn std::error::Error>> {
    let api_key = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config("chatgpt.model");

    let messages = vec![json!({
        "role": "user",
        "content": content,
    })];

    let functions = vec![
        json!({
            "name": "reply_latest_story",
            "description": "Get the latest story from daily Hacker News RSS feed",
            "parameters": {
                "type": "object",
                "properties": {
                    "token": {
                        "type": "string",
                        "description": "Channel access tokens as a means of authentication for the channel",
                    },
                    "reply_token": {
                        "type": "string",
                        "description": "Reply token that is used when sending a reply message"},
                },
                "required": ["token", "reply_token"],
            },
        }),
        json!({
            "name": "push_summary",
            "description": "Push the selected news (by index, start from 1, maximum is 10) summary to the user.",
            "parameters": {
                "type": "object",
                "properties": {
                    "token": {
                        "type": "string",
                        "description": "Channel access tokens as a means of authentication for the channel",
                    },
                    "user_id": {
                        "type": "string",
                        "description": "User ID of the target user"},
                    "index": {
                        "type": "integer",
                        "description": "Index of the news to be pushed to the user"},
                },
                "required": ["toekn", "user_id", "index"],
            },
        }),
    ];

    let payload = serde_json::to_string(&json!({
        "model": model,
        "messages": messages,
        "functions": functions,
        "function_call": "auto",
    }))?;

    let response = send_chat_request_json(api_key.as_str(), url.as_str(), payload).await?;

    println!("{}", response);
    let response_json: serde_json::Value = serde_json::from_str(&response)?;
    let function_call = if let Some(choices) = response_json["choices"].as_array() {
        if let Some(function_call) = choices[0]["message"]["function_call"].as_object() {
            let function_name = function_call["name"].as_str().unwrap();
            let function_args = function_call["arguments"].as_str().unwrap();

            Some(json!({
                "name": function_name,
                "arguments": function_args,
            }))
        } else {
            Some(json!({
                "message": choices[0]["message"]["content"].as_str().unwrap(),
            }))
        }
    } else {
        None
    };
    Ok(function_call.unwrap_or(json!({})).to_string())
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
                "這是今日的 Hacker News 前十大新聞，以綜合分析的方式進行概括，並條列出各新聞的主要重點。同時，請將各項新聞中最重要的一項與其相關的關鍵字突顯出來。最後，請以適當的段落劃分，並以('\n\n')作為分段符號。: {}",
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

pub async fn translate(content: String, language_code: String) -> Result<String, Box<dyn std::error::Error>> {
    let api_secret = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config("chatgpt.model");

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: format!("translate to {}: {}", language_code, content),
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

pub async fn get_language_code(text: String) -> Result<String, Box<dyn std::error::Error>> {
    let api_secret = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config("chatgpt.model");

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: format!("identify the input is which language, and response it only to ISO 639-1 standard language codes and country code without any more explaination: {}", text),
        }],
        temperature: 0.0,
        max_tokens: 2048,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
    };
    let res_content = send_chat_request(api_secret, url, request).await?;

    Ok(res_content)
}

async fn send_chat_request(
    api_secret: String,
    url: String,
    request: ChatRequest,
) -> Result<String, Box<dyn std::error::Error>> {
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

async fn send_chat_request_json(
    api_secret: &str,
    url: &str,
    payload: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let res = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_secret))
        .body(payload)
        .send().await?;
    Ok(res.text().await?)
}