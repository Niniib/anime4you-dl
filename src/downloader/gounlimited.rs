use crate::{anime4you::Host, downloader::Downloader};
use anyhow::{anyhow, Error};
use regex::Regex;

pub async fn new(url: &str) -> Result<Downloader, Error> {
    let request = reqwest::get(url).await?;
    let site_source = request.text().await?;
    let regex = Regex::new(r#"type\|(.*?)\|(.*?)'"#).unwrap();
    let captures = regex.captures(&site_source);
    if captures.is_none() {
        Err(anyhow!("Failed to retrieve sources."))?
    }
    let captures = captures.unwrap();
    let video_id = String::from(captures.get(1).unwrap().as_str());
    let fs_number = String::from(captures.get(2).unwrap().as_str());
    let video_url = format!("https://{}.gounlimited.to/{}/v.mp4", fs_number, video_id);
    Ok(Downloader {
        video_url,
        file_name: String::from("v.mp4"),
        host: Host::GoUnlimited
    })
}
