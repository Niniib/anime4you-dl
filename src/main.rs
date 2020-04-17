use std::fs::{create_dir, File};
use std::process::{exit, Command};
use std::str::FromStr;

use clap::{App, Arg};
use colored::Colorize;

use anyhow::{anyhow, Error};

use crate::series::{Host, Series, Synchronization};

mod downloader;
mod series;

fn main() {
    match run() {
        Ok(_) => {}
        Err(e) => eprintln!("{}", e.to_string()),
    }
}

fn run() -> Result<(), Error> {
    let matches = App::new("Anime4You downloader")
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            Arg::with_name("series_name")
                .short("n")
                .long("name")
                .takes_value(true)
                .conflicts_with("series_id")
                .help("Series name"),
        )
        .arg(
            Arg::with_name("series_id")
                .short("i")
                .long("id")
                .takes_value(true)
                .conflicts_with("series_name")
                .conflicts_with("gersub")
                .conflicts_with("gerdub")
                .help("Series id"),
        )
        .arg(
            Arg::with_name("gersub")
                .long("gersub")
                .short("s")
                .conflicts_with("gerdub")
                .help("Downloads the episodes with german subtitles and japanese synchronization."),
        )
        .arg(
            Arg::with_name("gerdub")
                .long("gerdub")
                .short("d")
                .conflicts_with("gersub")
                .help("Downloads the episodes with german synchronization."),
        )
        .arg(
            Arg::with_name("out")
                .long("out")
                .short("o")
                .takes_value(true)
                .help("Downloads the episodes in the specified path."),
        )
        .arg(
            Arg::with_name("file_pattern")
                .long("file-pattern")
                .short("p")
                .takes_value(true)
                .help("File name pattern for example: (%series_name)-Episode-(%episode) ** File extension will be automatically present"),
        )
        .arg(
            Arg::with_name("use_youtube_dl")
                .long("youtube-dl")
                .short("y")
                .takes_value(false)
                .help("Downloads the series with youtube-dl"),
        )
        .arg(
            Arg::with_name("episodes")
                .long("episodes")
                .short("e")
                .takes_value(true)
                .help("2,5 | Downloads episodes 2 through 5"),
        )
        .get_matches();
    let series: Series;
    let output: String;
    if matches.is_present("series_name") {
        let mut synchronization = Synchronization::Japanese;
        if matches.is_present("gersub") {
            synchronization = Synchronization::Japanese;
        } else if matches.is_present("gerdub") {
            synchronization = Synchronization::German;
        }
        series = Series::get_from_name(matches.value_of("series_name").unwrap(), &synchronization)
            .expect(
                format!(
                    "Could not fetch series with name \"{}\" and synchronization \"{}\"",
                    matches
                        .value_of("series_name")
                        .ok_or(anyhow!("Could not find \"series_name\" value"))
                        .unwrap(),
                    synchronization.get_name()
                )
                .as_str(),
            );
    } else if matches.is_present("series_id") {
        series = Series::get_from_id(matches.value_of("series_id").unwrap().parse().unwrap())
            .expect(
                format!(
                    "Could not fetch series with id \"{}\"",
                    matches
                        .value_of("series_id")
                        .ok_or(anyhow!("Could not find \"series_id\" value"))
                        .unwrap()
                )
                .as_str(),
            );
    } else {
        println!("Please use \"--help\"");
        exit(0);
    }

    if matches.is_present("out") {
        output = matches.value_of("out").unwrap().to_string();
    } else {
        output = format!("{}", series.id);
    }
    let _ = create_dir(&output);

    let mut use_youtube_dl = false;
    if matches.is_present("use_youtube_dl") {
        use_youtube_dl = true;
    }
    let mut arg_episodes: Vec<u32> = Vec::new();
    if matches.is_present("episodes") {
        let range: Vec<&str> = matches.value_of("episodes").unwrap().split(",").collect();
        let min = u32::from_str(range[0])?;
        let max = u32::from_str(range[1])?;
        for i in min..=max {
            arg_episodes.push(i)
        }
    } else {
        for i in 1..=series.episodes {
            arg_episodes.push(i);
        }
    }
    let episodes = series.get_episodes(&arg_episodes)?;
    let mut episode_count: u32 = arg_episodes[0];

    for link in episodes {
        let mut pattern = "(%series_name)-Episode-(%episode)".to_string();
        if matches.is_present("file_pattern") {
            pattern = matches.value_of("file_pattern").unwrap().to_string();
        }
        pattern = pattern.replace("(%series_name)", &series.title);
        pattern = pattern.replace("(%episode)", &episode_count.to_string());
        if use_youtube_dl {
            let _ = youtube_dl(link.as_str(), format!("{}/{}", output, pattern).as_str());
        } else {
            let link = link.as_str();
            let hoster = Host::get_from_name(link);
            let downloader = match hoster {
                Host::Vivo => Some(downloader::vivo::new(link)),
                Host::Vidoza => Some(downloader::vidoza::new(link)),
                Host::GoUnlimited => Some(downloader::gounlimited::new(link)),
                _ => None,
            };
            if downloader.is_none() {
                println!("{}", "Could not find suitable downloader! Please use the -y flag to enable youtube-dl".red());
                exit(1);
            }
            let downloader = downloader.unwrap();
            if downloader.is_err() {
                println!(
                    "{}",
                    format!("Failed to download from {}. Skipping...", link)
                        .as_str()
                        .red()
                );
                continue;
            }
            let downloader = downloader?;
            let episode_name = format!("{}/{}.{}", output, pattern, downloader.get_extension());
            println!(
                "{}",
                format!("Downloading {} => {}", downloader.get_url(), &episode_name)
                    .as_str()
                    .green()
            );
            let downloaded_episode = downloader.download_to_file(&mut File::create(&episode_name)?);
            if downloaded_episode.is_err() {
                println!(
                    "{}",
                    format!("Could not download episode {} -- skipped", episode_name)
                        .as_str()
                        .red()
                );
                continue;
            }
        }
        episode_count += 1;
    }

    Ok(())
}

fn youtube_dl(url: &str, output: &str) -> Result<(), Error> {
    let mut p = Command::new("youtube-dl");
    println!(
        "{}",
        format!("Downloading {}...", url).as_str().bright_green()
    );
    let cmd = p
        .arg(url)
        .arg("--output")
        .arg(format!("{}.%(ext)s", output).as_str())
        .output();
    if cmd.is_err() {
        println!("You did not install youtube-dl!");
        exit(1);
    }
    Ok(())
}
