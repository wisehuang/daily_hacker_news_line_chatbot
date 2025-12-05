use serde_json::json;
use url::Url;
use warp::{http::StatusCode, reject::Rejection, reply::Reply};

use crate::api::{chatgpt, kagi, line};
use crate::models::{ApiError, ApiResult, LineMessage};
use crate::rss;
use crate::utils::{self, CONFIG_CACHE};

/// Get latest HN stories and return as JSON
pub async fn get_latest_stories() -> Result<impl Reply, Rejection> {
    match rss::get_latest_stories().await {
        Ok(stories) => Ok(warp::reply::with_status(
            warp::reply::json(&stories),
            StatusCode::OK,
        )),
        Err(e) => {
            log::error!("Failed to get latest stories from RSS feed: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while fetching RSS feed",
                ApiError::ExternalServiceError(_) => "RSS feed service error or parsing failed",
                _ => "Failed to retrieve stories",
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": false, "error": error_msg})),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Get latest story title for testing
pub async fn get_latest_title() -> Result<impl Reply, Rejection> {
    match rss::fetch_feed().await {
        Ok(channel) => {
            if let Some(item) = rss::get_latest_item(&channel) {
                if let Some(title) = item.title() {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&json!({"success": true, "title": title})),
                        StatusCode::OK,
                    ));
                }
            }
            
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": false, "error": "No title found"})),
                StatusCode::NOT_FOUND,
            ))
        }
        Err(e) => {
            log::error!("Failed to fetch RSS feed for latest title: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while fetching RSS feed",
                ApiError::ExternalServiceError(_) => "RSS feed service unavailable or invalid format",
                _ => "Failed to get RSS feed",
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({"success": false, "error": error_msg})),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Get latest story as a LINE message
pub async fn get_latest_story_message() -> ApiResult<LineMessage> {
    let channel = rss::fetch_feed().await?;
    
    if let Some(item) = rss::get_latest_item(&channel) {
        // Extract story details
        let title = item.title()
            .ok_or_else(|| ApiError::ExternalServiceError("No title found".to_string()))?;
        let link = item.link()
            .ok_or_else(|| ApiError::ExternalServiceError("No link found".to_string()))?;
            
        // Format message
        let message_text = format!("Latest story: {}\n{}", title, link);
        
        // Return as LINE message
        Ok(line::create_text_message(&message_text))
    } else {
        Err(ApiError::ExternalServiceError("No stories found".to_string()))
    }
}

/// Broadcast today's stories to all LINE users
pub async fn send_line_broadcast() -> Result<impl Reply, Rejection> {
    // Get the stories
    let stories = match rss::get_latest_stories().await {
        Ok(stories) => stories,
        Err(e) => {
            log::error!("Failed to get stories for broadcast: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while fetching stories for broadcast",
                ApiError::ExternalServiceError(_) => "RSS feed service error during broadcast preparation",
                _ => "Failed to get stories for broadcast",
            };
            return Ok(utils::json_error(error_msg, StatusCode::INTERNAL_SERVER_ERROR));
        }
    };

    // Generate summaries for each story concurrently
    let summary_futures: Vec<_> = stories
        .iter()
        .map(|story| chatgpt::get_single_story_summary(story.story.clone()))
        .collect();

    let summaries = futures::future::join_all(summary_futures).await;

    // Pair stories with summaries, using default text for failed summaries
    let stories_with_summaries: Vec<_> = stories
        .into_iter()
        .zip(summaries.into_iter())
        .map(|(story, summary_result)| {
            let summary = summary_result.unwrap_or_else(|e| {
                log::warn!("Failed to get summary for story '{}': {:?}", story.story, e);
                "摘要生成失敗".to_string()
            });
            (story, summary)
        })
        .collect();

    // Create Flex carousel message
    let messages = vec![line::create_stories_carousel(&stories_with_summaries)];

    // Send broadcast
    let channel_token = &CONFIG_CACHE.channel_token;
    match line::broadcast_message(channel_token, messages).await {
        Ok(_) => Ok(utils::json_success()),
        Err(e) => {
            log::error!("Failed to broadcast stories via LINE API: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while sending broadcast to LINE",
                ApiError::ExternalServiceError(_) => "LINE API error during broadcast",
                _ => "Failed to broadcast stories",
            };
            Ok(utils::json_error(error_msg, StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// Broadcast a daily summary of Hacker News stories
pub async fn broadcast_daily_summary() -> Result<impl Reply, Rejection> {
    // Get the stories
    let stories = match rss::get_latest_stories().await {
        Ok(stories) => stories,
        Err(e) => {
            log::error!("Failed to get stories for daily summary: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while fetching stories for summary",
                ApiError::ExternalServiceError(_) => "RSS feed service error during summary preparation",
                _ => "Failed to get stories for summary",
            };
            return Ok(utils::json_error(error_msg, StatusCode::INTERNAL_SERVER_ERROR));
        }
    };
    
    // Format stories for summary
    let stories_text = stories
        .iter()
        .enumerate()
        .map(|(i, story)| format!("{}. {} {}", i + 1, story.story, story.storylink))
        .collect::<Vec<String>>()
        .join("\n\n");
    
    // Get summary from ChatGPT
    let summary = match chatgpt::get_chatgpt_summary(stories_text).await {
        Ok(summary) => summary,
        Err(e) => {
            log::error!("Failed to get summary from ChatGPT: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while contacting ChatGPT API",
                ApiError::AiError(_) => "ChatGPT API error during summary generation",
                ApiError::ExternalServiceError(_) => "ChatGPT service unavailable",
                _ => "Failed to create AI summary",
            };
            return Ok(utils::json_error(error_msg, StatusCode::INTERNAL_SERVER_ERROR));
        }
    };

    // Create Flex bubble message for summary
    let messages = vec![line::create_summary_bubble(&summary)];

    // Send broadcast
    let channel_token = &CONFIG_CACHE.channel_token;
    match line::broadcast_message(channel_token, messages).await {
        Ok(_) => Ok(utils::json_success()),
        Err(e) => {
            log::error!("Failed to broadcast summary via LINE API: {:?}", e);
            let error_msg = match e {
                ApiError::NetworkError(_) => "Network error while sending summary broadcast to LINE",
                ApiError::ExternalServiceError(_) => "LINE API error during summary broadcast",
                _ => "Failed to broadcast summary",
            };
            Ok(utils::json_error(error_msg, StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// Push summaries of specific stories to a user
pub async fn push_story_summaries(
    token: &str,
    user_id: &str,
    language_code: &str,
    indexes: &[usize],
) -> ApiResult<()> {
    // Get all stories
    let stories = rss::get_latest_stories().await?;
    
    // Filter stories by indexes
    let selected_stories: Vec<_> = indexes
        .iter()
        .filter_map(|&i| {
            if i > 0 && i <= stories.len() {
                Some(&stories[i - 1])
            } else {
                None
            }
        })
        .collect();
    
    if selected_stories.is_empty() {
        return Err(ApiError::ValidationError("No valid stories selected".to_string()));
    }
    
    // Format stories for summary
    let stories_text = selected_stories
        .iter()
        .enumerate()
        .map(|(i, story)| format!("{}. {} {}", i + 1, story.story, story.storylink))
        .collect::<Vec<String>>()
        .join("\n\n");
    
    // Get summary from ChatGPT
    let mut summary = chatgpt::get_chatgpt_summary(stories_text).await?;

    // Translate if needed
    if language_code != "en" && language_code != "zh-tw" {
        summary = chatgpt::translate(summary, language_code.to_string()).await?;
    }

    // Create Flex bubble message and send
    let message = line::create_summary_bubble(&summary);
    line::push_message(token, user_id, vec![message]).await
}

/// Push a summary of a URL to a user
pub async fn push_url_summary(
    token: &str,
    user_id: &str,
    language_code: &str,
    url: &str,
) -> ApiResult<()> {
    // Validate URL
    if !is_valid_url(url) {
        return Err(ApiError::ValidationError("Invalid URL provided".to_string()));
    }

    // Get summary from Kagi
    let mut summary = kagi::summarize_url(url).await?;

    // Translate if needed
    if language_code != "en" && language_code != "zh-tw" {
        summary = chatgpt::translate(summary, language_code.to_string()).await?;
    }

    // Create Flex bubble message and send
    let message = line::create_summary_bubble(&summary);
    line::push_message(token, user_id, vec![message]).await
}

/// Validate if a URL is properly formatted and uses http/https
fn is_valid_url(url: &str) -> bool {
    match Url::parse(url) {
        Ok(parsed_url) => {
            // Only allow http and https schemes
            matches!(parsed_url.scheme(), "http" | "https") && 
            // Ensure there's a host
            parsed_url.host().is_some()
        }
        Err(_) => false,
    }
} 