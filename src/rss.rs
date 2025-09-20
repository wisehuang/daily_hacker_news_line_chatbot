use rss::{Channel, Item};
use scraper::{Html, Selector};

use crate::models::{ApiResult, Story, ApiError};
use crate::utils::{HTTP_CLIENT, CONFIG_CACHE, with_retry};

/// Fetch RSS feed and parse it
pub async fn fetch_feed() -> ApiResult<Channel> {
    with_retry(|| async {
        let feed_url = &CONFIG_CACHE.rss_feed_url;
        
        // Fetch the RSS content
        let response = HTTP_CLIENT.get(feed_url)
            .send()
            .await
            .map_err(ApiError::NetworkError)?;
        
        // Check if the response was successful
        if !response.status().is_success() {
            return Err(ApiError::ExternalServiceError(
                format!("Failed to fetch RSS feed: HTTP {}", response.status())
            ));
        }
        
        // Get the bytes from the response
        let content = response.bytes()
            .await
            .map_err(ApiError::NetworkError)?;
        
        // Parse the RSS channel
        Channel::read_from(&content[..])
            .map_err(|e| ApiError::ExternalServiceError(format!("Failed to parse RSS: {}", e)))
    }).await
}

/// Get the latest item from the RSS channel
pub fn get_latest_item(channel: &Channel) -> Option<Item> {
    channel.items().first().cloned()
}

/// Get the latest Hacker News stories
pub async fn get_latest_stories() -> ApiResult<Vec<Story>> {
    // Fetch and parse the RSS feed
    let channel = fetch_feed().await?;
    
    // Get the first item's description (which contains the HTML with story links)
    let description = channel.items()
        .first()
        .and_then(|item| item.description())
        .ok_or_else(|| ApiError::ExternalServiceError("No stories found in RSS feed".to_string()))?;

    // Parse the HTML to extract stories
    let html = Html::parse_document(description);
    
    // Extract story links and titles; Daemonology renders each entry as
    // `<a class="storylink" href=...>Title</a>` so target the anchor itself
    let selector = match Selector::parse("a.storylink") {
        Ok(selector) => selector,
        Err(_) => return Err(ApiError::ExternalServiceError("Failed to parse HTML selector".to_string())),
    };

    // Build the story list
    let stories: Vec<Story> = html
        .select(&selector)
        .filter_map(|element| {
            let href = element.value().attr("href")?;
            let title = element.text().collect::<String>();
            
            Some(Story {
                storylink: href.to_owned(),
                story: title,
            })
        })
        .collect();
    
    // Return empty list with error if no stories were found
    if stories.is_empty() {
        return Err(ApiError::ExternalServiceError("No stories found in RSS feed".to_string()));
    }
    
    Ok(stories)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_feed() {
        let result = fetch_feed().await;
        assert!(result.is_ok(), "Should successfully fetch RSS feed");
    }

    #[tokio::test]
    async fn test_get_latest_stories() {
        let stories = get_latest_stories().await;
        assert!(stories.is_ok(), "Should successfully get stories");
        
        let stories = stories.unwrap();
        assert!(!stories.is_empty(), "Should have at least one story");
        
        let first_story = &stories[0];
        assert!(!first_story.story.is_empty(), "Story title should not be empty");
        assert!(!first_story.storylink.is_empty(), "Story link should not be empty");
    }
} 
