use anyhow::Error;
use std::fs::File;

pub mod gounlimited;
pub mod vidoza;
pub mod vivo;

/*
 * https://github.com/Fludixx/serienstream-dl/blob/master/src/downloader/mod.rs
 * fludixx
 */

pub struct Downloader {
    url: String,
    video_url: String,
    file_name: String,
}

impl Downloader {
    pub fn get_url(&self) -> String {
        self.url.clone()
    }

    pub fn get_extension(&self) -> String {
        String::from(self.file_name.split(".").last().unwrap())
    }

    pub fn download_to_file(&self, file: &mut File) -> Result<(), Error> {
        let mut video = reqwest::get(&self.video_url)?;
        video.copy_to(file)?;
        Ok(())
    }
}
