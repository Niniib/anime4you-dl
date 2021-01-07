use anyhow::{anyhow, Error};
use regex::Regex;
use reqwest::header::HeaderValue;

#[derive(Clone, Debug)]
pub struct CookieJar {
    pub cookies: Vec<Cookie>,
}

impl CookieJar {
    pub fn new() -> CookieJar {
        CookieJar {
            cookies: Vec::new(),
        }
    }

    pub fn add_cookie(&mut self, cookie: Cookie) {
        if cookie.value != String::from("deleted") {
            if let Some(duplicated_index) = self.cookies.iter().position(|c| c.key == cookie.key) {
                self.cookies.remove(duplicated_index);
            };
            self.cookies.push(cookie);
        }
    }

    pub fn serialize(&self) -> String {
        let mut buffer: String = String::new();
        for value in &self.cookies {
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
