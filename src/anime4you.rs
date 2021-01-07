use anyhow::{anyhow, Error};
use regex::Regex;
use reqwest::{
    multipart::{self, Part},
    Client,
};

use crate::cookie::CookieJar;

#[derive(Clone, Debug)]
// priority
#[repr(u32)]
pub enum Host {
    Vivo = 2,
    Vidoza = 3,
    GoUnlimited = 1,
    Unknown = 0,
}

const SITE: &str = "https://www.anime4you.one";
const CAPTCHA_SITE: &str = "https://captcha.anime4you.one";
const ANIME_LIST: &str = "/speedlist.old.txt";

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
pub enum Language {
    JapaneseWithGermanSubtitles,
    German,
    Other(String),
}

impl Language {
    pub fn get_name(&self) -> &str {
        match self {
            Language::German => "gerdub",
            Language::JapaneseWithGermanSubtitles => "gersub",
            Language::Other(other) => other.as_str(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Series {
    pub id: u32,
    pub title: String,
    pub episodes: u32,
    pub language: Language,
}

impl Series {
    pub async fn get_from_id(id: u32) -> Result<Series, Error> {
        let client = Client::new();
        let response = client
            .get(format!("{}/show/1/aid/{}", SITE, id).as_str())
            .send()
            .await?;
        let response_text = response.text().await?;
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
            language: match synchronization {
                "GerSub" => Language::JapaneseWithGermanSubtitles,
                "GerDub" => Language::German,
                other => Language::Other(other.to_string()),
            },
        };
        Ok(series)
    }

    pub async fn get_from_name(name: &str, synchronization: &Language) -> Result<Series, Error> {
        let client = Client::new();
        let resp = client
            .get(&format!("{}{}", SITE, ANIME_LIST))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
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
                    .to_owned(),
                episodes: found
                    .get("Folgen")
                    .ok_or(anyhow!("Series has no \"Folgen\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"Folgen\" value not a string"))?
                    .parse()?,
                language: match found
                    .get("Untertitel")
                    .ok_or(anyhow!("Series has no \"Untertitel\" value"))?
                    .as_str()
                    .ok_or(anyhow!("Series \"Untertitel\" value not a string"))?
                {
                    "gersub" => Language::JapaneseWithGermanSubtitles,
                    "gerdub" => Language::German,
                    other => Language::Other(other.to_string()),
                },
            };
            Ok(series)
        } else {
            Err(anyhow!("Series \"{}\" not found", name))
        }
    }
}

#[derive(Clone, Debug)]
pub struct Captcha {
    pub session: String,
    pub id_prefix: String,
    pub question: String,
    pub images: Vec<String>,
}

impl Captcha {
    pub fn new(
        session: String,
        id_prefix: String,
        question: String,
        images: Vec<String>,
    ) -> Captcha {
        Captcha {
            session,
            id_prefix,
            question,
            images,
        }
    }
}

pub struct Resolver {
    pub series: Series,
    pub cookies: CookieJar,
    pub client: Client,
}

impl Resolver {
    pub fn from_series(series: Series) -> Resolver {
        let cookies = CookieJar::new();
        let client = Client::new();
        Resolver {
            series,
            cookies,
            client,
        }
    }

    pub async fn populate_cookies(&mut self, episode: u32) -> Result<(), Error> {
        let cookies_request = self
            .client
            .get(
                format!(
                    "{}/show/1/aid/{}/epi/{}/#vidplayer",
                    SITE, self.series.id, episode
                )
                .as_str(),
            )
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
            )
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header("Dnt", "1")
            .header("Connection", "keep-alive")
            .send()
            .await?;
        if !cookies_request.status().is_success() {
            Err(anyhow!("Failed to fetch cookies."))?
        }
        self.cookies = CookieJar::parse(
            cookies_request
                .headers()
                .get_all("set-cookie")
                .iter()
                .collect(),
        )?;
        Ok(())
    }

    pub async fn get_captcha(&mut self, episode: u32) -> Result<Captcha, Error> {
        let captcha_request = self
            .client
            .get(format!("{}/Captcheck/api.php?action=new", CAPTCHA_SITE).as_str())
            //.header("Cookie", self.cookies.serialize())
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
            )
            .send()
            .await?;

        let json = captcha_request.json::<serde_json::Value>().await?;
        let question_regex = Regex::new(r#".*:{0,1}\s(.*)\."#)?;
        let mut images = Vec::new();
        for answer in json
            .get("answers")
            .ok_or(anyhow!("Invalid captcha response: Missing field"))?
            .as_array()
            .ok_or(anyhow!("Invalid captcha response: Wrong type"))?
        {
            images.push(
                answer
                    .as_str()
                    .ok_or(anyhow!("Invalid captcha response: Wrong type"))?
                    .to_owned(),
            );
        }
        Ok(Captcha::new(
            json.get("session")
                .ok_or(anyhow!("Invalid captcha response: Missing field"))?
                .as_str()
                .ok_or(anyhow!("Invalid captcha response: Wrong type"))?
                .to_owned(),
            json.get("id_prefix")
                .ok_or(anyhow!("Invalid captcha response: Missing field"))?
                .as_str()
                .ok_or(anyhow!("Invalid captcha response: Wrong type"))?
                .to_owned(),
            question_regex
                .captures_iter(
                    json.get("question_i")
                        .ok_or(anyhow!("Invalid captcha response: Missing field"))?
                        .as_str()
                        .ok_or(anyhow!("Invalid captcha response: Wrong type"))?,
                )
                .next()
                .ok_or(anyhow!("Could not extract question"))?
                .get(1)
                .ok_or(anyhow!("Could not match question"))?
                .as_str()
                .to_owned(),
            images,
        ))
    }

    pub async fn download_captcha_image(
        &self,
        episode: u32,
        captcha: &Captcha,
        image_hash: &str,
    ) -> Result<Vec<u8>, Error> {
        let image_request = self
            .client
            .get(
                format!(
                    "{}/Captcheck/api.php?action=img&s={}&c={}",
                    CAPTCHA_SITE,
                    captcha.session.as_str(),
                    image_hash
                )
                .as_str(),
            )
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
            )
            .send()
            .await?
            .bytes()
            .await?;
        Ok(image_request.iter().map(|b| *b).collect())
    }

    pub async fn submit_captcha_image(
        &self,
        episode: u32,
        captcha: &Captcha,
        image_hash: &str,
    ) -> Result<Option<String>, Error> {
        let form = multipart::Form::new()
            .part("aid", Part::stream(self.series.id.to_string()))
            .part("epi", Part::stream(episode.to_string()))
            .part("username", Part::stream(""))
            .part(
                "captcheck_selected_answer",
                Part::stream(image_hash.to_string()),
            )
            .part(
                "captcheck_session_code",
                Part::stream(captcha.session.to_string()),
            );

        let captcha_request = self
            .client
            .post(format!("{}/Captcheck/humancheck.php", SITE).as_str())
            .header("Cookie", self.cookies.serialize())
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Referer",
                format!("{}/show/1/aid/{}/epi/{}", SITE, self.series.id, episode).as_str(),
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
            )
            .multipart(form)
            .send()
            .await?
            .text()
            .await?;
        Ok(if captcha_request.starts_with("FALSE") {
            None
        } else {
            Some(captcha_request)
        })
    }

    pub async fn extract_links(&self, response_text: &str) -> Result<Vec<String>, Error> {
        let vivo_regex = Regex::new(r#"<button href='(.+)' data-src"#)?;
        let alternative_regex = Regex::new(r#"<button data-src='([^<]*)' class"#)?;
        let mut links = Vec::new();
        if vivo_regex.is_match(response_text) {
            links.push(
                vivo_regex
                    .captures_iter(response_text)
                    .next()
                    .ok_or(anyhow!("Failed to match vivo regex"))?
                    .get(1)
                    .ok_or(anyhow!("Regex did not find vivo link"))?
                    .as_str()
                    .to_string(),
            );
        }
        if alternative_regex.is_match(response_text) {
            for capture in alternative_regex.captures_iter(response_text) {
                let response = self
                    .client
                    .post(&format!("{}/check_video.php", SITE))
                    .form(&[("vidhash", capture.get(1).unwrap().as_str())])
                    .send()
                    .await?;
                links.push(response.text().await?.trim().to_string());
            }
        }
        // sort by priority (defined in Enum)
        links.sort_by(|a, b| (Host::get_from_name(b) as u32).cmp(&(Host::get_from_name(a) as u32)));
        Ok(links)
    }
}
