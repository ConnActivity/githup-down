use chrono::prelude::*;
use reqwest::header::HeaderValue;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;

pub async fn get_image_for_repo(user: impl AsRef<str>, repo: impl AsRef<str>) -> String {
    let utc: DateTime<Utc> = Utc::now();
    let formatted_time = utc.format("%Y-%m-%d %H:%M:%S").to_string();
    let mut hasher = Sha256::new();
    hasher.update(formatted_time);
    let hash_digest = hasher.finalize();
    let hashed_time = format!("{:x}", hash_digest);

    format!(
        "https://opengraph.githubassets.com/{}/{}/{}",
        hashed_time,
        user.as_ref(),
        repo.as_ref()
    )
}
