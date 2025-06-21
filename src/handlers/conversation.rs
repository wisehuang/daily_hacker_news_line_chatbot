use bytes::Bytes;
use serde_json::{json, Value};
use warp::{http::StatusCode, reject::Rejection, reply::Reply};

use crate::api::chatgpt;

/// Handle conversation requests
pub async fn handler(content: Bytes) -> Result<impl Reply, Rejection> {
    // Convert bytes to string
    let conversation = match String::from_utf8(content.to_vec()) {
        Ok(text) => text,
        Err(e) => {
            log::error!("Failed to parse conversation content: {}", e);
            return Ok(warp::reply::with_status(
                warp::reply::json(&json!({"error": "Invalid UTF-8 content"})),
                StatusCode::BAD_REQUEST,
            ));
        }
    };
    
    // Process the conversation with ChatGPT
    let response = match chatgpt::run_conversation(conversation).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("ChatGPT error: {:?}", e);
            return Ok(warp::reply::with_status(
                warp::reply::json(&json!({"error": "Failed to process conversation"})),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    
    // Parse the function call response
    let function_call: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(e) => {
            log::error!("Failed to parse ChatGPT response: {}", e);
            return Ok(warp::reply::with_status(
                warp::reply::json(&json!({"error": "Failed to parse ChatGPT response"})),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    log::info!("Function call: {}", function_call);

    // Format response based on function call type
    match function_call.get("name").and_then(Value::as_str) {
        Some(function_name) => {
            // Extract arguments if available
            let arguments_value = function_call["arguments"].as_str().unwrap_or("{}");
            let arguments: Value = match serde_json::from_str(arguments_value) {
                Ok(value) => value,
                Err(_) => json!({})
            };

            // Log specific information for certain function calls
            if function_name == "push_summary" {
                if let Some(indexes) = arguments["indexes"].as_array() {
                    log::info!("Pushing summaries for indexes: {:?}", indexes);
                }
            }

            // Return the function call data
            Ok(warp::reply::with_status(
                warp::reply::json(&function_call),
                StatusCode::OK,
            ))
        }
        None => {
            // Return the message content if no function call
            let response = json!({
                "message": function_call["message"].as_str().unwrap_or("No message content"),
            });
            
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
    }
} 