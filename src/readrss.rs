use scraper::{Html, Selector};
use rss::{Channel, Item};
use serde::{Deserialize, Serialize};
use std::error::Error;
use crate::config_helper::get_config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub storylink: String,
    pub story: String,
}

pub async fn read_feed() -> Result<Channel, Box<dyn Error>> {
    let url = get_config("rss.feed_url");
    let content = reqwest::get(url)
        .await?
        .bytes()
        .await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

pub fn get_latest_item(channel: &rss::Channel) -> Option<Item> {
    channel.items().first().map(|item| item.clone())
}

pub async fn get_last_hn_stories() -> Vec<Story> {
    let channel = read_feed()
        .await
        .unwrap_or_else(|err| panic!("read RSS failed: {}", err));
    let _description = channel.items()[0].description().unwrap();

    // Parse the HTML description to get the story links and titles
    let html = Html::parse_document(_description);
    let storylink_selector = Selector::parse(".storylink a").unwrap();
    let stories = html
        .select(&storylink_selector)
        .filter_map(|storylink| {
            let href = storylink.value().attr("href")?;
            let title = storylink.text().collect::<String>();
            Some(Story {
                storylink: href.to_owned(),
                story: title,
            })
        })
        .collect();
    stories
}
