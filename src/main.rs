use serde_json::json;
use warp::http::Error;
use warp::reply::Json;
use warp::Filter;

// Module declarations
mod api {
    pub mod line;
    pub mod chatgpt;
    pub mod kagi;
}
mod config;
mod handlers;
mod models;
mod rss;
mod utils;

// Re-exports for easier access
use handlers::webhook;
use handlers::stories;
use handlers::conversation;

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init();

    // Parse webhook requests
    let parse_request_route = warp::post()
        .and(warp::path("webhook"))
        .and(warp::header::<String>("x-line-signature"))
        .and(warp::body::bytes())
        .and_then(webhook::parse_request_handler);

    // Testing endpoint
    let test_route = warp::get()
        .and(warp::path("hello"))
        .map(|| Ok::<Json, Error>(warp::reply::json(&json!({"success": true}))));

    // Story endpoints
    let latest_title_route = warp::get()
        .and(warp::path("getLatestTitle"))
        .and_then(stories::get_latest_title);

    let get_stories_route = warp::get()
        .and(warp::path("getLatestStories"))
        .and_then(stories::get_latest_stories);

    let send_line_broadcast_route = warp::get()
        .and(warp::path("sendTodayStories"))
        .and_then(stories::send_line_broadcast);

    let broadcast_daily_summary_route = warp::get()
        .and(warp::path("broadcastDailySummary"))
        .and_then(stories::broadcast_daily_summary);

    // Conversation endpoint
    let conversation_route = warp::post()
        .and(warp::path("conversation"))
        .and(warp::body::bytes())
        .and_then(conversation::handler);

    // Add request logging
    let log_filter = warp::log("daily_hacker_news_bot");

    // Combine all routes
    let routes = parse_request_route
        .or(test_route)
        .or(latest_title_route)
        .or(get_stories_route)
        .or(send_line_broadcast_route)
        .or(broadcast_daily_summary_route)
        .or(conversation_route)
        .with(log_filter);

    // Start the server
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
