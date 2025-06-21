use serde::{Deserialize, Serialize};
use serde_json::json;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::config;
use crate::models::{ApiError, ApiResult};
use crate::utils::{HTTP_CLIENT, CONFIG_CACHE};

/// ChatGPT request with messages
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

/// Message in a chat conversation
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response from ChatGPT API
#[derive(Debug, Deserialize)]
struct ChatCompletion {
    id: String,
    object: String,
    created: i64,
    model: String,
    usage: Usage,
    choices: Vec<Choice>,
}

/// Token usage information
#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

/// One choice from the model
#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
    finish_reason: String,
    index: i32,
}

/// Message from the model
#[derive(Debug, Deserialize)]
struct Message {
    role: String,
    content: String,
}

/// Function calling response structures
#[derive(Debug, Deserialize)]
struct FunctionCallResponse {
    choices: Vec<FunctionChoice>,
}

#[derive(Debug, Deserialize)]
struct FunctionChoice {
    message: FunctionMessage,
}

#[derive(Debug, Deserialize)]
struct FunctionMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    function: ToolFunction,
}

#[derive(Debug, Deserialize)]
struct ToolFunction {
    name: String,
    arguments: String,
}

/// Our internal function call result
#[derive(Debug, Serialize)]
struct FunctionCallResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Process a user message with function calling capabilities
pub async fn run_conversation(content: String) -> ApiResult<String> {
    let api_key = &CONFIG_CACHE.chatgpt_secret;
    let url = &CONFIG_CACHE.chat_completions_url;
    let model = &CONFIG_CACHE.chatgpt_model;

    // Create user message
    let messages = vec![json!({
        "role": "user",
        "content": content,
    })];

    // Define available functions
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

    // Create the request payload
    let payload = serde_json::to_string(&json!({
        "model": model,
        "messages": messages,
        "tools": functions,
        "tool_choice": "auto",
    }))?;

    // Send the request
    let response = HTTP_CLIENT
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .body(payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        log::error!("ChatGPT API error: HTTP {}", status);
        return Err(ApiError::ExternalServiceError(format!("AI service error: {}", status)));
    }

    let response = response.text().await?;

    // Parse the response with typed structs
    log::info!("Response from function calling: {}", response);
    let function_response: FunctionCallResponse = serde_json::from_str(&response)?;
    
    // Extract function call or message content
    let first_choice = function_response.choices.first()
        .ok_or_else(|| ApiError::AiError("No choices in response".to_string()))?;
    
    let result = if let Some(tool_calls) = &first_choice.message.tool_calls {
        if let Some(first_tool) = tool_calls.first() {
            FunctionCallResult {
                name: Some(first_tool.function.name.clone()),
                arguments: Some(first_tool.function.arguments.clone()),
                message: None,
            }
        } else {
            FunctionCallResult {
                name: None,
                arguments: None,
                message: first_choice.message.content.as_ref().map(|s| s.clone()),
            }
        }
    } else {
        FunctionCallResult {
            name: None,
            arguments: None,
            message: first_choice.message.content.as_ref().map(|s| s.clone()),
        }
    };

    let tool_choice_json = serde_json::to_string(&result)?;
    
    log::info!("Function call: {}", tool_choice_json);
    Ok(tool_choice_json)
}

/// Get a response from ChatGPT using a prompt template
pub async fn get_chatgpt_response(
    prompt_key: &str, 
    content: String, 
    temperature: f64, 
    use_translate_model: bool
) -> ApiResult<String> {
    let api_secret = &CONFIG_CACHE.chatgpt_secret;
    let url = &CONFIG_CACHE.chat_completions_url;
    let model = if use_translate_model {
        &CONFIG_CACHE.chatgpt_translate_model
    } else {
        &CONFIG_CACHE.chatgpt_model
    };
    let prompt = config::get_prompt(prompt_key);

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
    
    send_chat_request(api_secret.to_owned(), url.to_owned(), request).await
}

/// Generate a summary of Hacker News stories
pub async fn get_chatgpt_summary(stories: String) -> ApiResult<String> {
    get_chatgpt_response("prompt.summary_all", stories, 0.05, false).await
}

/// Detect the language of text
pub async fn get_language_code(text: String) -> ApiResult<String> {
    get_chatgpt_response("prompt.get_language_code", text, 0.0, false).await
}

/// Translate content to a specific language
pub async fn translate(content: String, language_code: String) -> ApiResult<String> {
    let content = format!("{}: {}", language_code, content);
    get_chatgpt_response("prompt.translate", content, 0.05, true).await
}

/// Send a chat request with a structured request object
async fn send_chat_request(
    api_secret: String,
    url: String,
    request: ChatRequest,
) -> ApiResult<String> {
    let json_body = serde_json::to_string(&request)?;

    let response = HTTP_CLIENT
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_secret))
        .body(json_body)
        .send()
        .await?;

    if !response.status().is_success() {
        // Get status code for logging but don't include the full error message
        // which might contain sensitive information
        let status = response.status();
        log::error!("ChatGPT API error: HTTP {}", status);
        return Err(ApiError::ExternalServiceError(format!("AI service error: {}", status)));
    }

    let response_text = response.text().await?;
    let completion: ChatCompletion = serde_json::from_str(&response_text)?;

    if let Some(choice) = completion.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err(ApiError::AiError("No response from AI service".to_string()))
    }
}