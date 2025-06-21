use std::fmt;
use warp::{Rejection, Reply};
use warp::http::StatusCode;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    InvalidUtf8(std::string::FromUtf8Error),
    JsonParse(serde_json::Error),
    Network(reqwest::Error),
    ChatGpt(String),
    Kagi(String),
    Config(String),
    LineApi(String),
    InvalidSignature,
    MissingField(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::InvalidUtf8(e) => write!(f, "Invalid UTF-8: {}", e),
            AppError::JsonParse(e) => write!(f, "JSON parse error: {}", e),
            AppError::Network(e) => write!(f, "Network error: {}", e),
            AppError::ChatGpt(msg) => write!(f, "ChatGPT error: {}", msg),
            AppError::Kagi(msg) => write!(f, "Kagi error: {}", msg),
            AppError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AppError::LineApi(msg) => write!(f, "LINE API error: {}", msg),
            AppError::InvalidSignature => write!(f, "Invalid signature"),
            AppError::MissingField(field) => write!(f, "Missing required field: {}", field),
        }
    }
}

impl std::error::Error for AppError {}

impl warp::reject::Reject for AppError {}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "Not Found";
    } else if let Some(app_error) = err.find::<AppError>() {
        match app_error {
            AppError::InvalidSignature => {
                code = StatusCode::UNAUTHORIZED;
                message = "Invalid signature";
            }
            AppError::JsonParse(_) | AppError::InvalidUtf8(_) | AppError::MissingField(_) => {
                code = StatusCode::BAD_REQUEST;
                message = "Bad request";
            }
            _ => {
                code = StatusCode::INTERNAL_SERVER_ERROR;
                message = "Internal server error";
            }
        }
    } else if err.find::<warp::filters::body::BodyDeserializeError>().is_some() {
        code = StatusCode::BAD_REQUEST;
        message = "Invalid body";
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "Method not allowed";
    } else {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal server error";
    }

    let json = json!({
        "success": false,
        "error": message
    });

    Ok(warp::reply::with_status(warp::reply::json(&json), code))
}