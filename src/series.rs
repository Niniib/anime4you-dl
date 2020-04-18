use anyhow::{anyhow, Error};
use colored::Colorize;
use image::Rgba;
use image::{load_from_memory_with_format, ImageFormat};
use regex::Regex;
use reqwest::header::HeaderValue;
use reqwest::Client;
use std::collections::HashMap;
use std::io::Read;
use std::process::exit;
use std::thread;
use std::time::Duration;

const SITE: &str = "https://www.anime4you.one";
const CAPTCHA_SITE: &str = "https://captcha.anime4you.one";
const CAPTCHA_REQUEST: &str = "/src/captcha-request.php";
const CHECK_CAPTCHA: &str = "/check_captcha.php";
const ANIME_LIST: &str = "/speedlist.old.txt";

#[derive(Clone, Debug)]
pub enum Host {
    Vivo,
    Vidoza,
    GoUnlimited,
    Unknown,
}

impl Host {
    pub fn get_from_name(link: &str) -> Host {
        let regex = Regex::new(r#"https://(.*?)/"#).unwrap();
        let mut hoster = Host::Unknown;
        if let Some(capture) = regex.captures_iter(link).next() {
            hoster = match capture
                .get(1)
                .ok_or(anyhow!("regex capture does not have valid string result"))
                .unwrap()
                .as_str()
            {
                "vivo.sx" => Host::Vivo,
                "gounlimited.to" => Host::GoUnlimited,
                "vidoza.net" => Host::Vidoza,
                _ => Host::Unknown,
            }
        }
        hoster
    }
}

#[derive(Clone, Debug)]
pub enum Synchronization {
    Japanese,
    German,
    Other(String),
}

impl Synchronization {
    pub fn get_name(&self) -> &str {
        match self {
            Synchronization::German => "gerdub",
            Synchronization::Japanese => "gersub",
            Synchronization::Other(other) => other.as_str(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CookieJar {
    pub cookies: HashMap<String, Cookie>,
}

impl CookieJar {
    pub fn new() -> CookieJar {
        CookieJar {
            cookies: HashMap::new(),
        }
    }

    pub fn add_cookie(&mut self, cookie: Cookie) {
        if cookie.value != String::from("deleted") {
            self.cookies.insert(cookie.key.clone(), cookie);
        }
    }

    pub fn serialize(&self) -> String {
        let mut buffer: String = String::new();
        for (_, value) in self.cookies.clone() {
            buffer.push_str(&value.serialize());
            buffer.push_str("; ");
        }
        buffer.chars().take(buffer.chars().count() - 2).collect()
    }

    pub fn parse(headers: Vec<&HeaderValue>) -> Result<CookieJar, Error> {
        let mut cookie_jar = CookieJar::new();
        let header_value_regex = Regex::new(r#"(.*?)=(.*?);"#).unwrap();
        for header_value in headers {
            let header_captures = header_value_regex
                .captures(header_value.to_str()?)
                .expect("Not valid header");
            let header_key = header_captures
                .get(1)
                .ok_or(anyhow!("Not valid header key"))
                .unwrap()
                .as_str();
            let header_value = header_captures
                .get(2)
                .ok_or(anyhow!("Not valid header value"))
                .unwrap()
                .as_str();
            cookie_jar.add_cookie(Cookie {
                key: header_key.to_string(),
                value: header_value.to_string(),
            });
        }
        Ok(cookie_jar)
    }

    pub fn _update(&mut self, cookie_jar: CookieJar) {
        for cookie in cookie_jar.cookies {
            self.add_cookie(cookie.1);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cookie {
    pub key: String,
    pub value: String,
}

impl Cookie {
    pub fn serialize(&self) -> String {
        format!("{}={}", self.key, self.value)
    }
}

#[derive(Clone, Debug)]
pub struct Series {
    pub id: u32,
    pub title: String,
    pub episodes: u32,
    pub synchronization: Synchronization,
}

impl Series {
    pub fn get_from_id(id: u32) -> Result<Series, Error> {
        let client = Client::new();
        let mut response = client
            .get(format!("{}/show/1/aid/{}", SITE, id).as_str())
            .send()?;
        let response_text = response.text()?;
        let episodes_regex = Regex::new(r#"href="(/show/1/aid/\d+/epi/\d+/)"#).unwrap();
        let mut episodes: Vec<&str> = Vec::new();
        for capture in episodes_regex.captures_iter(response_text.as_str()) {
            episodes.push(capture.get(0).unwrap().as_str())
        }
        let title_regex = Regex::new(r#"<h3 class="cpfont6">([^<]*)<"#).unwrap();
        let mut title: &str = "";
        for capture in title_regex.captures_iter(response_text.as_str()) {
            title = capture.get(1).unwrap().as_str();
        }
        let synchronization_regex = Regex::new(r#"<h5 class="cpfont6 pt-3">([^<]*)&"#).unwrap();
        let mut synchronization: &str = "unknown";
        for capture in synchronization_regex.captures_iter(response_text.as_str()) {
            synchronization = capture.get(1).unwrap().as_str();
        }
        let series = Series {
            id,
            title: title.to_string(),
            episodes: episodes.len() as u32,
            synchronization: match synchronization {
                "GerSub" => Synchronization::Japanese,
                "GerDub" => Synchronization::German,
                other => Synchronization::Other(other.to_string()),
            },
        };
        println!(
            "{}",
            format!("[*] Found series \"{}\"", series.title)
                .as_str()
                .green()
        );
        Ok(series)
    }

    pub fn get_from_name(name: &str, synchronization: &Synchronization) -> Result<Series, Error> {
        let client = Client::new();
        let resp = client
            .get(&format!("{}{}", SITE, ANIME_LIST))
            .send()?
            .json::<serde_json::Value>()?;
        let mut found = None;
        for x in resp
            .as_array()
            .ok_or(anyhow!("API response isn't an array"))?
            .iter()
        {
            let series = x
                .as_object()
                .ok_or(anyhow!("API response array element not an object"))?;

            if series
                .get("titel")
                .ok_or(anyhow!(
                    "API response array element key \"titel\" doesn't exist"
                ))?
                .as_str()
                .ok_or(anyhow!(
                    "API response array element key \"titel\" is not a string"
                ))?
                .to_lowercase()
                .contains(&name.to_lowercase())
            {
                if series
                    .get("Untertitel")
                    .ok_or(anyhow!("Series has no \"Untertitel\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"Untertitel\" value not a string"))?
                    == synchronization.get_name()
                {
                    found = Option::Some(series);
                    break;
                }
            }
        }
        if let Some(found) = found {
            let series = Series {
                id: found
                    .get("aid")
                    .ok_or(anyhow!("Series has no \"aid\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"aid\" value not a string"))?
                    .parse()?,
                title: found
                    .get("titel")
                    .ok_or(anyhow!("Series has no \"titel\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"titel\" value not a string"))?
                    .to_string(),
                episodes: found
                    .get("Folgen")
                    .ok_or(anyhow!("Series has no \"Folgen\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"Folgen\" value not a string"))?
                    .parse()?,
                synchronization: match found
                    .get("Untertitel")
                    .ok_or(anyhow!("Series has no \"Untertitel\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"Untertitel\" value not a string"))?
                {
                    "gersub" => Synchronization::Japanese,
                    "gerdub" => Synchronization::German,
                    other => Synchronization::Other(other.to_string()),
                },
            };
            println!(
                "{}",
                format!("[*] Found series \"{}\"", series.title)
                    .as_str()
                    .green()
            );
            Ok(series)
        } else {
            Err(anyhow!("Series \"{}\" not found", name))
        }
    }

    pub fn get_episodes(&self, range: &Vec<u32>) -> Result<Vec<String>, Error> {
        let mut episodes: Vec<String> = Vec::new();
        let vivo_regex = Regex::new(r#"<button href='(.+)' data-src"#).unwrap(); // vivo link regex
        let alternative_regex = Regex::new(r#"<button data-src='([^<]*)' class"#).unwrap(); // alternative host regex
        let client = Client::new();
        let min = *range
            .get(0)
            .ok_or(anyhow!("Can not fetch first range number"))?;
        let max;
        if range.len() != 1 {
            max = *range
                .get(range.len() - 1)
                .ok_or(anyhow!("Can not fetch second range number"))?;
        } else {
            max = min;
        }
        'outer: for episode_count in min..=max {
            loop {
                println!("{}", "Waiting 10 seconds before continuing".purple());
                thread::sleep(Duration::from_secs(10));
                println!("{}", "[*] Fetching Cookies".yellow());
                let cookies_request = client
                .get(&format!(
                    "{}/show/1/aid/{}/epi/{}/#vidplayer",
                    SITE, self.id, episode_count
                ))
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (X11; Linux x86_64; rv:74.0) Gecko/20100101 Firefox/74.0",
                )
                .header(
                    "Referer",
                    &format!("{}/show/1/aid/{}/epi/{}", SITE, self.id, episode_count),
                )
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
                )
                .header("Dnt", "1")
                .header("Connection", "keep-alive")
                .send();
                let cookies_request = cookies_request?;
                if cookies_request.status().is_success() {
                    println!("{}", "\t[*] Successfully fetched cookies".green());
                } else {
                    println!("{}", "\t[!] Failed to fetch cookies. Next episode...".red());
                    continue 'outer;
                }
                let cookie_jar = CookieJar::parse(
                    cookies_request
                        .headers()
                        .get_all("set-cookie")
                        .iter()
                        .collect(),
                )?;
                let mut captcha_site = client
                .get(&format!(
                    "{}/index.php?aid={}&epi={}",
                    CAPTCHA_SITE, self.id, episode_count
                ))
                .header("Cookie", cookie_jar.serialize())
                .header(
                    "Referer",
                    &format!("{}/show/1/aid/{}/epi/{}", SITE, self.id, episode_count),
                )
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
                )
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (X11; Linux x86_64; rv:74.0) Gecko/20100101 Firefox/74.0",
                ).send()?;
                let captcha_site = captcha_site.text()?;
                let ip_regex =
                    Regex::new(r#"<input type="hidden" value="(.*?)" name="ip">"#).unwrap();
                let captures = ip_regex
                    .captures(captcha_site.as_str())
                    .ok_or(anyhow!("Failed to fetch ip"))?;
                let ip = captures.get(1).unwrap().as_str();
                println!("{}", "[*] Trying captcha".yellow());
                println!("{}", "\t[*] Fetching captcha hashes".yellow());
                let value = client
                    .post(&format!("{}{}", CAPTCHA_SITE, CAPTCHA_REQUEST))
                    .header("Referer", &format!("{}{}", CAPTCHA_SITE, CAPTCHA_REQUEST))
                    .header("Dnt", "1")
                    .header("X-Requested-With", "XMLHttpRequest")
                    .header("Cookie", cookie_jar.serialize())
                    .form(&[("cID", "0"), ("rT", "1"), ("tM", "light")])
                    .send()?
                    .json::<serde_json::Value>();
                if value.is_err() {
                    println!("{}", "\t[!] Fetching captcha hashes failed".red());
                    println!("{}", "[!] Captcha failed!".red());
                    exit(1);
                }
                let value = value?;
                let mut images = Vec::new();
                let mut alpha_values = Vec::new();
                for hash in value.as_array().ok_or(anyhow!("Invalid hash response"))? {
                    let hash_request = client
                        .get(&format!(
                            "{}{}?cid=0&hash={}",
                            CAPTCHA_SITE,
                            CAPTCHA_REQUEST,
                            hash.as_str().ok_or(anyhow!("Invalid hash value"))?
                        ))
                        .header("Cookie", cookie_jar.serialize())
                        .header("Referer", "https://captcha.anime4you.one/index.php")
                        .header("Accept", "image/webp,*/*")
                        .header(
                            "User-Agent",
                            "Mozilla/5.0 (X11; Linux x86_64; rv:74.0) Gecko/20100101 Firefox/74.0",
                        )
                        .send()?;
                    let bytes = hash_request.bytes();
                    images.push(load_from_memory_with_format(
                        bytes
                            .map(|b| b.unwrap_or(0u8))
                            .collect::<Vec<u8>>()
                            .as_ref(),
                        ImageFormat::Png,
                    )?);
                    println!(
                        "\t\t{}",
                        format!("[*] Got capture icon hash {}", hash)
                            .as_str()
                            .green()
                    );
                }
                for (i, image) in images.into_iter().enumerate() {
                    alpha_values.push((
                        i,
                        image
                            .as_rgba8()
                            .unwrap()
                            .pixels()
                            .map(|&Rgba([_, _, _, a])| a as u64)
                            .sum::<u64>(),
                    ));
                }
                alpha_values.sort_by_key(|&(_, a)| a);
                let answer_hash = value
                    .get(
                        if (alpha_values[0].1 as i64 - alpha_values[1].1 as i64).abs()
                            > (alpha_values[3].1 as i64 - alpha_values[4].1 as i64).abs()
                        {
                            alpha_values[0].0
                        } else {
                            alpha_values[4].0
                        },
                    )
                    .ok_or(anyhow!("Captcha hashes does not have values"))?
                    .as_str()
                    .ok_or(anyhow!("Captcha hash is not valid string"))?;
                let _response = client
                    .post(&format!("{}{}", CAPTCHA_SITE, CAPTCHA_REQUEST))
                    .header("X-Requested-With", "XMLHttpRequest")
                    .header("Referer", &format!("{}{}", CAPTCHA_SITE, CAPTCHA_REQUEST))
                    .header("Origin", CAPTCHA_SITE)
                    .header("Host", "captcha.anime4you.one")
                    .header("TE", "Trailers")
                    .header(
                        "User-Agent",
                        "Mozilla/5.0 (X11; Linux x86_64; rv:74.0) Gecko/20100101 Firefox/74.0",
                    )
                    .header(
                        "Content-Type",
                        "application/x-www-form-urlencoded; charset=UTF-8",
                    )
                    .header("Content-Length", "62")
                    .header("Cookie", cookie_jar.serialize())
                    .header("Connection", "keep-alive")
                    .header("Accept", "*/*")
                    .form(&[("cID", "0"), ("pC", answer_hash), ("rT", "2")])
                    .send()?;

                let mut vkey_request = client
                    .post(&format!("{}{}", CAPTCHA_SITE, CHECK_CAPTCHA))
                    .header("Cookie", cookie_jar.serialize())
                    .form(&[
                        ("captcha-hf", answer_hash),
                        ("captcha-idhf", "0"),
                        ("aid", self.id.to_string().as_str()),
                        ("epi", episode_count.to_string().as_str()),
                        ("ip", ip),
                    ])
                    .send()?;

                let api_response = vkey_request.json::<serde_json::Value>();
                if api_response.is_err() {
                    println!("{}", "[!] Failed to solve captcha! Trying again.".red());
                    // Failed captcha, redoing...
                    continue;
                }
                let api_response = api_response?;

                let vkey = api_response
                    .get("hw")
                    .ok_or(anyhow!("Unable to get vkey"))?
                    .as_str()
                    .ok_or(anyhow!("Unable to get vkey as string"))?;
                println!("{}", "[*] Resolved captcha!".green());

                let mut response = client
                    .post(&format!("{}/check_hoster.php", SITE))
                    .form(&[
                        ("epi", &episode_count.to_string()),
                        ("aid", &self.id.to_string()),
                        ("act", &episode_count.to_string()),
                        ("vkey", &vkey.to_string()),
                        ("username", &String::new()),
                    ])
                    .send()?;
                let response_text = response.text()?;
                if vivo_regex.is_match(&response_text) {
                    // vivo
                    for capture in vivo_regex.captures_iter(&response_text) {
                        let vivo_link = capture
                            .get(1)
                            .ok_or(anyhow!("Regex did not find vivo link"))?
                            .as_str()
                            .to_string();
                        episodes.push(vivo_link.clone());
                        println!(
                            "{}",
                            format!("[*] Fetched episode {}. ({})", episode_count, vivo_link)
                                .as_str()
                                .green()
                        );
                    }
                    continue 'outer;
                }

                if alternative_regex.is_match(&response_text) {
                    // other hosters
                    if let Some(capture) = alternative_regex.captures_iter(&response_text).next() {
                        let mut response = client
                            .post(&format!("{}/check_video.php", SITE))
                            .form(&[("vidhash", capture.get(1).unwrap().as_str())])
                            .send()?;
                        let response_text = response.text()?.trim().to_string();
                        println!(
                            "{}",
                            format!("[*] Fetched episode {}. ({})", episode_count, response_text)
                        );
                        episodes.push(response_text);
                    }
                    continue 'outer;
                }
            }
        }
        Ok(episodes)
    }
}
