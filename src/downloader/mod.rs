use std::fs::File;

use anyhow::Error;

use crate::anime4you::Host;

pub mod streamtape;
pub mod gounlimited;
pub mod vidoza;
pub mod vivo;

pub struct Downloader {
    pub video_url: String,
    pub file_name: String,
    pub host: Host,
}

impl Downloader {
    pub fn get_extension(&self) -> String {
        String::from(self.file_name.split(".").last().unwrap())
    }

    pub async fn download_to_file(&self, mut file: File) -> Result<(), Error> {
        let video_url = self.video_url.clone();
        tokio::task::spawn_blocking(move || -> Result<(), Error> {
            let mut video = reqwest::blocking::get(video_url.as_str())?;
            video.copy_to(&mut file)?;
            Ok(())
        }).await??;
        Ok(())
    }
}
