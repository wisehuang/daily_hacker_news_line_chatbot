use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::config_helper::{get_config, get_prompt};
use crate::json;

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
            "type": "function",
            "function": {
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
        }}),
        json!({
            "type": "function",
            "function": {
            "name": "push_summary",
            "description": "In the ChatGPT function call, push the selected news summary to the user by index (starting from 1, with a maximum index of 10). The index is passed as an array of integers, with a maximum array size of 5. If the array size exceeds 5, please return error without calling this function.",
            "parameters": {
                "type": "object",
                "properties": {
                    "indexes": {
                        "type": "array",
                        "description": "An array of integers, with a maximum array size of 5, representing the indices of news articles that will be sent to the user.",
                        "items": {
                            "type": "integer"
                          },
                    },
                },
                "required": ["indexes"],
            },
        }}),
        json!({
            "type": "function",
            "function": {
            "name": "push_url_summary",
            "description": "In the ChatGPT function call, push the content summary to the user URL.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "An URL of a web page, which content will be summarized and push the summary to user.",
                    },
                },
                "required": ["url"],
            },
        }}),
    ];

    let payload = serde_json::to_string(&json!({
        "model": model,
        "messages": messages,
        "tools": functions,
        "tool_choice": "auto",
    }))?;

    let response = send_chat_request_json(api_key.as_str(), url.as_str(), payload).await?;

    log::info!("response from function calling: {}", response);
    let response_json: serde_json::Value = serde_json::from_str(&response)?;
    let function_call = if let Some(choices) = response_json["choices"].as_array() {
        if let Some(function_call) = choices[0]["message"]["tool_calls"][0]["function"].as_object() {
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

    let tool_choice_json = function_call.unwrap_or(json!({})).to_string();
    log::info!("function_call: {}", tool_choice_json);
    Ok(tool_choice_json)
}

pub async fn get_chatgpt_response(prompt_key: &str, content: String, temperature: f64, model_key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let api_secret = get_config("chatgpt.secret");
    let url = get_config("chatgpt.chat_completions_url");
    let model = get_config(model_key);
    let prompt = get_prompt(prompt_key);

    let request = ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: format!("{} {}", prompt, content),
        }],
        temperature,
        max_tokens: 2048,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
    };
    let res_content = send_chat_request(api_secret, url, request).await?;
    Ok(res_content)
}

pub async fn get_chatgpt_summary(stories: String) -> Result<String, Box<dyn std::error::Error>> {
    get_chatgpt_response("prompt.summary_all", stories, 0.05, "chatgpt.model").await
}

pub async fn get_language_code(text: String) -> Result<String, Box<dyn std::error::Error>> {
    get_chatgpt_response("prompt.get_language_code", text, 0.0, "chatgpt.model").await
}

pub async fn translate(content: String, language_code: String) -> Result<String, Box<dyn std::error::Error>> {
    let content = format!("{}: {}", language_code, content);
    get_chatgpt_response("prompt.translate", content, 0.05, "chatgpt.translate_model").await
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

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use super::*;

    #[tokio::test]
    async fn test_run_conversation() {
        let content = "第一, 第二, 第三".to_string();
        let result = run_conversation(content).await.unwrap();
        println!("result: {}", result);
        let expected_result = r#"{"arguments":"{\n  \"indexes\": [1,2,3]\n}","name":"push_summary"}"#;
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn test_url_summary() {
        let url = "https://www.apple.com/apple-music/".to_string();
        let result = run_conversation(url).await.unwrap();
        println!("result: {}", result);
        let json: Result<Value, _> = serde_json::from_str(result.as_str());
        let url = match json {
            Ok(data) => {
                let arguments = data.get("arguments").unwrap().as_str().unwrap();
                println!("arguments_str: {}", arguments);
                let arguments_json: Value = serde_json::from_str(arguments).unwrap();
                let url_str = arguments_json.get("url").unwrap().as_str().unwrap().to_string();
                url_str
            },
            Err(e) => panic!("Problem with JSON: {:?}", e),
        };
        let expected_result = r#"https://www.apple.com/apple-music/"#;
        assert_eq!(url, expected_result);
    }

    #[tokio::test]
    async fn test_get_language_code() {
        let text = "Hello, world!".to_string();
        let result = get_language_code(text).await;

        assert_eq!(result.unwrap(), "en-us");
    }

    #[tokio::test]
    async fn test_translate() {
        let content = "Hello, world!".to_string();
        let language_code = "es".to_string(); // Spanish language code
        let result = translate(content, language_code).await;

        assert_eq!(result.unwrap(), "¡Hola, mundo!");
    }
}
