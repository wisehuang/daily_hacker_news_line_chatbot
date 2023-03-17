use serde_json::json;
use warp::Filter;

mod r#mod;
mod chatgpt;
mod readrss;
mod request_handler;
mod config_helper;
mod line_helper;

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init();    

    let parse_request_route = warp::post()
        .and(warp::path("webhook"))
        .and(warp::header::<String>("x-line-signature"))
        .and(warp::body::bytes())        
        .and_then(r#mod::parse_request_handler);

    let test_route = warp::get()
        .and(warp::path("hello"))
        .map(|| Ok(warp::reply::json(&json!({"success": true}))));

    let latest_title_route = warp::get()
    .and(warp::path("getLatestTitle"))
    .and_then(r#mod::get_latest_title);
    
    let get_stories_route = warp::get()
    .and(warp::path("getLatestStories"))
    .and_then(r#mod::get_latest_stories);

    let send_line_broadcast_route = warp::get()
    .and(warp::path("sendTodayStories"))
    .and_then(r#mod::send_line_broadcast);

    let log_filter = warp::log("daily_hacker_news_bot");

    let routes = parse_request_route
    .or(test_route)
    .or(latest_title_route)
    .or(get_stories_route)
    .or(send_line_broadcast_route)
    .with(log_filter);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
