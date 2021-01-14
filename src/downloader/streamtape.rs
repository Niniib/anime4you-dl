use crate::{anime4you::Host, downloader::Downloader};
use anyhow::{anyhow, Error};
use regex::Regex;

pub async fn new(url: &str) -> Result<Downloader, Error> {
    let request = reqwest::get(url).await?;
    let site_source = request.text().await?;
    let video_regex = Regex::new(
        r#"document\.getElementById\('.*'\+'.*'\)\.innerHTML\s=\s"(.*)"\s\+\s'(.*)'"#,
    )?;
    let name_regex = Regex::new(r#"<title>(.*)\sat\sStreamtape.com</title>"#)?;
    let file_name = String::from(
        name_regex
            .captures(site_source.as_str())
            .ok_or(anyhow!("Unable to extract file name"))?
            .get(1)
            .ok_or(anyhow!("Unable to extract file name"))?
            .as_str(),
    );
    let video_url_capture = video_regex
        .captures(site_source.as_str())
        .ok_or(anyhow!("Unable to extract video url"))?;

    let video_url = format!(
        "{}{}",
        video_url_capture
            .get(1)
            .ok_or(anyhow!("Unable to extract video url"))?
            .as_str(),
        video_url_capture
            .get(2)
            .ok_or(anyhow!("Unable to extract video url"))?
            .as_str()
    ).replace("//", "https://");

    Ok(Downloader {
        video_url,
        file_name,
        host: Host::Streamtape,
    })
}
