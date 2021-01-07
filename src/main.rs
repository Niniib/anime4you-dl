use std::{
    collections::HashMap,
    fs::File,
    path::Path,
    process::{exit, Command},
};

use anime4you::{Host, Language, Resolver, Series};
use anyhow::{anyhow, Error};
use clap::{App, Arg};
use colorful::Color;
use colorful::Colorful;
use downloader::Downloader;
use dssim_core::{Dssim, ToRGBAPLU};
use imgref::Img;
use rustbreak::{deser::Bincode, FileDatabase};
use tokio::time::{sleep, Duration};

mod anime4you;
mod cookie;
mod downloader;

fn is_number(test: String) -> Result<(), String> {
    test.parse::<u32>().map_err(|err| err.to_string())?;
    Ok(())
}

fn is_range(test: String) -> Result<(), String> {
    let split: Vec<String> = test.split(",").map(|s| s.to_owned()).collect();
    if split.len() > 2 {
        Err(String::from("More than 2 numbers were entered"))?
    }
    for range in split {
        is_number(range.to_owned())?
    }
    Ok(())
}

fn done(log: &str) {
    println!("{} {}", "[+]".color(Color::Green), log.color(Color::Green))
}

fn fail(log: &str) {
    println!("{} {}", "[!]".color(Color::Red), log.color(Color::Red))
}

fn pending(log: &str) {
    println!(
        "{} {}",
        "[-]".color(Color::Yellow),
        log.color(Color::Yellow)
    )
}

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    let matches = App::new("anime4you-dl")
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            Arg::with_name("series_name")
                .long("name")
                .short("n")
                .takes_value(true)
                .conflicts_with("series_id")
                .required_unless("series_id")
                .value_name("NAME")
                .help("Searches anime4you by series name."),
        )
        .arg(
            Arg::with_name("series_id")
                .long("id")
                .short("i")
                .takes_value(true)
                .conflicts_with("series_name")
                .conflicts_with("gersub")
                .conflicts_with("gerdub")
                .required_unless("series_name")
                .validator(is_number)
                .value_name("ID")
                .help("Identifies the series by id."),
        )
        .arg(
            Arg::with_name("gersub")
                .long("gersub")
                .short("s")
                .conflicts_with("gerdub")
                .help("Downloads the episodes with japanese audio and german subtitles."),
        )
        .arg(
            Arg::with_name("gerdub")
                .long("gerdub")
                .short("d")
                .conflicts_with("gersub")
                .help("Downloads the episodes with german audio."),
        )
        .arg(
            Arg::with_name("out")
                .long("out")
                .short("o")
                .takes_value(true)
                .value_name("DIRECTORY")
                .help("Downloads the episodes to the specified path."),
        )
        .arg(
            Arg::with_name("file_pattern")
                .long("file-pattern")
                .short("p")
                .takes_value(true)
                .value_name("PATTERN")
                .help("File name pattern, i.e.: (%series_name)-Episode-(%episode) (File extension is added automatically)."),
        )
        .arg(
            Arg::with_name("youtube_dl")
                .long("youtube-dl")
                .short("y")
                .takes_value(false)
                .help("Uses youtube-dl from PATH to download."),
        )
        .arg(
            Arg::with_name("episodes")
                .long("episodes")
                .short("e")
                .takes_value(true)
                .validator(is_range)
                .value_name("RANGE")
                .help("Downloads episodes by a given range, i.e. 2,5 will download episodes 2 through 5."),
        )
        .arg(
            Arg::with_name("parallel")
                .long("parallel")
                .help("Downloads and resolves links simultaneously."),
        )
        .arg(
            Arg::with_name("delay")
                .long("delay")
                .takes_value(true)
                .default_value("5000")
                .value_name("DELAY")
                .validator(is_number)
                .help("The delay in milliseconds between each episode download"),
        )
        .get_matches();

    let series = if matches.is_present("series_name") {
        let mut language = Language::JapaneseWithGermanSubtitles;
        if matches.is_present("gersub") {
            language = Language::JapaneseWithGermanSubtitles;
        } else if matches.is_present("gerdub") {
            language = Language::German;
        }
        Series::get_from_name(matches.value_of("series_name").unwrap(), &language).await?
    } else if matches.is_present("series_id") {
        Series::get_from_id(matches.value_of("series_id").unwrap().parse().unwrap()).await?
    } else {
        unreachable!()
    };
    let range = if matches.is_present("episodes") {
        matches
            .value_of("episodes")
            .unwrap()
            .split(",")
            .map(|n| n.parse::<u32>().unwrap())
            .collect::<Vec<u32>>()
    } else {
        vec![1, series.episodes]
    };
    let output = if matches.is_present("output") {
        matches.value_of("output").unwrap().to_string()
    } else {
        format!("{} ({})", series.title.as_str(), series.id)
    };
    let _ = tokio::fs::create_dir(output.as_str()).await;
    done(format!("Found series \"{}\".", &series.title).as_str());
    let mut resolver = Resolver::from_series(series);
    let mut episode: u32 = range[0];
    let mut path = std::env::current_exe()?.ancestors().collect::<Vec<&Path>>()[1].to_path_buf();
    path.push("db.bin");
    let db =
        FileDatabase::<HashMap<String, Vec<u8>>, Bincode>::load_from_path_or_default(path)?;
    let mut handels = Vec::new();
    loop {
        resolver.populate_cookies(episode).await?;
        done(format!("Fetched cookies for Episode {}.", episode).as_str());
        let captcha = resolver.get_captcha(episode).await?;
        let mut images: Vec<Vec<u8>> = Vec::with_capacity(4);
        for image_hash in &captcha.images {
            images.push(
                resolver
                    .download_captcha_image(episode, &captcha, image_hash)
                    .await?,
            );
        }
        let image = db.read(|db| {
            db.get(captcha.question.as_str())
                .map_or(None, |b| Some(b.clone()))
        })?;
        let link = if image.is_none() {
            pending("Submitting random captcha.");
            let response = resolver
                .submit_captcha_image(episode, &captcha, &captcha.images[0])
                .await?;
            if response.is_none() {
                fail("Captcha submission was wrong, reloading...");
                None
            } else {
                done("Captcha submission was correct, saving in local database.");
                db.write(|db| {
                    db.insert(captcha.question, images[0].clone());
                })?;
                db.save()?;
                let mut link = None;
                for loop_link in resolver.extract_links(response.unwrap().as_str()).await? {
                    if get_downloader(loop_link.as_str()).await.is_ok() {
                        link = Some(loop_link);
                    }
                }
                link
            }
        } else {
            let attr = Dssim::new();
            let image = image.unwrap();
            done("Found captcha in database.");
            let src_image = lodepng::decode32(&image)?;
            let src_image = attr
                .create_image(&Img::new(
                    src_image.buffer.to_rgbaplu(),
                    src_image.width,
                    src_image.height,
                ))
                .ok_or(anyhow!("Failed to create original image"))?;
            let mut diffs = Vec::new();
            pending("Searching for most similar image...");
            for image in &images {
                let compare_image = lodepng::decode32(image)?;
                let compare_image = attr
                    .create_image(&Img::new(
                        compare_image.buffer.to_rgbaplu(),
                        compare_image.width,
                        compare_image.height,
                    ))
                    .ok_or(anyhow!("Failed to create comparison image"))?;
                let (diff, _) = attr.compare(&src_image, compare_image);
                diffs.push(diff);
            }
            if let Some(min) = diffs.iter().min_by(|a, b| a.partial_cmp(b).unwrap()) {
                let pos = diffs.iter().position(|v| min.eq(v)).unwrap();
                done(format!("Found similar image with {}", min).as_str());
                let response = resolver
                    .submit_captcha_image(episode, &captcha, captcha.images[pos].as_str())
                    .await?;
                if response.is_none() {
                    fail("Captcha submission was wrong, reloading...");
                    None
                } else {
                    done("Captcha submission was correct.");
                    let mut link = None;
                    for loop_link in resolver.extract_links(response.unwrap().as_str()).await? {
                        if get_downloader(loop_link.as_str()).await.is_ok() {
                            link = Some(loop_link);
                            break;
                        }
                    }
                    link
                }
            } else {
                None
            }
        };
        episode += 1;
        if let Some(link) = link {
            let mut pattern = "(%series_name)-Episode(%episode)".to_string();
            if matches.is_present("file_pattern") {
                pattern = matches.value_of("file_pattern").unwrap().to_string();
            }
            let use_youtube_dl = matches.is_present("youtube-dl");
            if matches.is_present("parallel") {
                let output = output.clone();
                let title = resolver.series.title.clone();
                handels.push(tokio::task::spawn(async move {
                    let _ = download(
                        episode - 1,
                        link.as_str(),
                        output.as_str(),
                        pattern,
                        title.as_str(),
                        use_youtube_dl,
                    )
                    .await;
                }));
            } else {
                let _ = download(
                    episode - 1,
                    link.as_str(),
                    output.as_str(),
                    pattern,
                    resolver.series.title.as_str(),
                    use_youtube_dl,
                )
                .await;
            }
        } else {
            fail(format!("No hoster avabile for episode {}.", episode - 1).as_str());
        }
        if episode >= range[1] {
            break;
        }
        sleep(Duration::from_millis(
            matches.value_of("delay").unwrap().parse::<u64>().unwrap(),
        ))
        .await;
    }
    for handle in handels {
        handle.await?;
    }
    Ok(())
}

fn youtube_dl(url: &str, output: &str) -> Result<(), Error> {
    let mut p = Command::new("youtube-dl");
    pending(format!("Downloading {} via youtube-dl", url).as_str());
    let cmd = p
        .arg(url)
        .arg("--output")
        .arg(format!("{}.%(ext)s", output).as_str())
        .output();
    if cmd.is_err() {
        eprintln!("An Error occured while trying to download via youtube-dl, please ensure that youtube-dl is installed and is in your PATH.");
        exit(1);
    }
    Ok(())
}

async fn get_downloader(link: &str) -> Result<Downloader, Error> {
    let hoster = Host::get_from_name(link);
    match hoster {
        Host::Vivo => downloader::vivo::new(link).await,
        Host::Vidoza => downloader::vidoza::new(link).await,
        Host::GoUnlimited => downloader::gounlimited::new(link).await,
        _ => Err(anyhow!("Host is unsupported."))?,
    }
}

async fn download(
    episode: u32,
    link: &str,
    output: &str,
    pattern: String,
    title: &str,
    use_youtube_dl: bool,
) -> Result<(), Error> {
    let pattern = pattern.replace("(%series_name)", title);
    let pattern = pattern.replace("(%episode)", episode.to_string().as_str());
    let pattern = format!("{}/{}", output, pattern);
    if use_youtube_dl {
        youtube_dl(link, pattern.as_str())?;
    } else {
        let hoster = Host::get_from_name(link);
        let downloader = match hoster {
            Host::Vivo => Some(downloader::vivo::new(link).await),
            Host::Vidoza => Some(downloader::vidoza::new(link).await),
            Host::GoUnlimited => Some(downloader::gounlimited::new(link).await),
            _ => None,
        };
        if downloader.is_none() {
            fail("No hoster was found.");
            return Ok(());
        }
        let downloader = downloader.unwrap();
        if downloader.is_err() {
            fail("An error occured while trying to download an episode.");
            return Ok(());
        }
        let downloader = downloader?;
        let pattern = format!("{}.{}", pattern, downloader.get_extension());
        pending(
            format!(
                "Downlaoding episode {} from {:?}...",
                episode, downloader.host
            )
            .as_str(),
        );
        if let Err(_) = downloader
            .download_to_file(File::create(pattern.as_str())?)
            .await
        {
            fail("Failed to download episode.");
        }
    }
    Ok(())
}
