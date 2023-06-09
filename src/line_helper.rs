use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::Bytes;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::error::Error;

use crate::config_helper::get_config;

#[derive(Serialize, Deserialize)]
pub struct LineMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct LineBroadcastRequest {
    pub messages: Vec<LineMessage>,
}

#[derive(Serialize, Deserialize)]
pub struct LineMessageRequest {
    pub replyToken: String,
    pub messages: Vec<LineMessage>,
}

#[derive(Deserialize, Debug)]
pub struct LineErrorResponse {
    pub message: String,
    pub details: Vec<LineErrorDetail>,
}

#[derive(Deserialize, Debug)]
pub struct LineErrorDetail {
    pub message: String,
    pub property: String,
}

#[derive(Serialize, Deserialize)]
pub struct LineSendMessageRequest {
    pub to: String,
    pub messages: Vec<LineMessage>,
}

pub fn generate_signature(channel_secret: &str, body: &[u8]) -> String {
    let mut hmac_sha256 =
        Hmac::<Sha256>::new_from_slice(channel_secret.as_bytes()).expect("Failed to create HMAC");
    hmac_sha256.update(&body);

    BASE64.encode(hmac_sha256.finalize().into_bytes())
}

pub fn is_signature_valid(x_line_signature: String, body: &Bytes) -> Result<(), Box<dyn Error>> {
    let channel_secret = get_config("channel.secret");

    log::info!("channel secret: {}", channel_secret);

    let encoded_body = generate_signature(&channel_secret, &body);

    log::info!("encoded body: {}", encoded_body);
    log::info!("x-line-signature: {:?}", x_line_signature);
    log::info!(
        "body content: {}",
        String::from_utf8(body.to_vec()).unwrap()
    );

    if encoded_body != x_line_signature {
        return Err("Invalid signature".into());
    }

    Ok(())
}