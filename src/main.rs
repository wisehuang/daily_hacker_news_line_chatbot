use serde_json::json;
use warp::http::Error;
use warp::reply::Json;
use warp::Filter;

mod chatgpt;
mod config_helper;
mod errors;
mod kagi;
mod line_helper;
mod handler;
mod readrss;
mod request_handler;

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init();

    let parse_request_route = warp::post()
        .and(warp::path("webhook"))
        .and(warp::header::<String>("x-line-signature"))
        .and(warp::body::bytes())
        .and_then(handler::parse_request_handler);

    let test_route = warp::get()
        .and(warp::path("hello"))
        .map(|| Ok::<Json, Error>(warp::reply::json(&json!({"success": true}))));

    let latest_title_route = warp::get()
        .and(warp::path("getLatestTitle"))
        .and_then(handler::get_latest_title);

    let get_stories_route = warp::get()
        .and(warp::path("getLatestStories"))
        .and_then(handler::get_latest_stories);

    let send_line_broadcast_route = warp::get()
        .and(warp::path("sendTodayStories"))
        .and_then(handler::send_line_broadcast);

    let broadcast_daily_summary_route = warp::get()
        .and(warp::path("broadcastDailySummary"))
        .and_then(handler::broadcast_daily_summary);

    let conversation_route = warp::post()
        .and(warp::path("conversation"))
        .and(warp::body::bytes())
        .and_then(handler::conversation_handler);

    let log_filter = warp::log("daily_hacker_news_bot");

    let routes = parse_request_route
        .or(test_route)
        .or(latest_title_route)
        .or(get_stories_route)
        .or(send_line_broadcast_route)
        .or(broadcast_daily_summary_route)
        .or(conversation_route)
        .with(log_filter)
        .recover(errors::handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
