use crate::downloader::Downloader;
use anyhow::{anyhow, Error};
use regex::Regex;

pub fn new(url: &str) -> Result<Downloader, Error> {
    let mut request = reqwest::get(url)?;
    let site_source = request.text()?;
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
        url: String::from(url),
        video_url,
        file_name: format!("v.mp4"),
    })
}
