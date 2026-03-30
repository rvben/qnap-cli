use anyhow::{bail, Context, Result};
use reqwest::{Client, ClientBuilder};

use crate::config::Config;

pub struct QnapClient {
    http: Client,
    base_url: String,
    pub sid: Option<String>,
}

impl QnapClient {
    pub fn new(config: &Config) -> Result<Self> {
        let host = config.host()?;
        let insecure = config.insecure.unwrap_or(false);

        let http = ClientBuilder::new()
            .danger_accept_invalid_certs(insecure)
            .cookie_store(true)
            .build()?;

        let base_url = if host.starts_with("http") {
            host.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", host.trim_end_matches('/'))
        };

        Ok(Self {
            http,
            base_url,
            sid: None,
        })
    }

    pub async fn login(&mut self, username: &str, password: &str) -> Result<String> {
        let hash = format!("{:x}", md5::compute(password));
        let url = format!(
            "{}/cgi-bin/authLogin.cgi?user={}&passwd={}&a=login",
            self.base_url,
            urlencoding::encode(username),
            urlencoding::encode(&hash)
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("failed to reach NAS")?;

        let body = resp.text().await?;

        // Parse the XML-like response to extract authSid
        let sid = extract_xml_value(&body, "authSid")
            .context("no authSid in login response")?;
        let passed = extract_xml_value(&body, "authPassed")
            .unwrap_or_default();

        if passed != "1" {
            let err = extract_xml_value(&body, "errorValue").unwrap_or_default();
            bail!("authentication failed (errorValue={})", err);
        }

        self.sid = Some(sid.clone());
        Ok(sid)
    }

    pub async fn get_cgi(&self, path: &str, params: &[(&str, &str)]) -> Result<String> {
        let sid = self.sid.as_deref().unwrap_or_default();
        let mut url = format!("{}{}", self.base_url, path);

        let mut query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();
        query.push(format!("sid={}", sid));
        url.push('?');
        url.push_str(&query.join("&"));

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("request failed")?;

        resp.text().await.context("failed to read response")
    }

    pub async fn get_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let sid = self.sid.as_deref().unwrap_or_default();
        let mut query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();
        query.push(format!("sid={}", sid));

        let url = format!("{}{}?{}", self.base_url, path, query.join("&"));

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("request failed")?;

        resp.json::<T>().await.context("failed to parse JSON response")
    }

}

/// Extract the text content of a simple XML-like tag: `<tag>content</tag>`
pub fn extract_xml_value(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&close)?;
    Some(body[start..start + end].to_string())
}
