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
        // QNAP QTS expects: pwd=base64(utf8(password)), sent as form POST
        use base64::{engine::general_purpose::STANDARD, Engine};
        let pwd = STANDARD.encode(password.as_bytes());
        let url = format!("{}/cgi-bin/authLogin.cgi", self.base_url);

        let resp = self
            .http
            .post(&url)
            .form(&[
                ("user", username),
                ("pwd", pwd.as_str()),
                ("serviceKey", "1"),
                ("client_app", "qnap-cli"),
            ])
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("certificate") || e.to_string().contains("validity") {
                    anyhow::anyhow!(
                        "TLS certificate error — retry with `--insecure` to skip verification\n  ({})",
                        e
                    )
                } else {
                    anyhow::anyhow!("failed to reach NAS: {}", e)
                }
            })?;

        let body = resp.text().await?;

        let passed = extract_xml_value(&body, "authPassed").unwrap_or_default();
        if passed != "1" {
            let err = extract_xml_value(&body, "errorValue").unwrap_or_default();
            bail!("authentication failed (errorValue={})", err);
        }

        let sid = extract_xml_value(&body, "authSid").ok_or_else(|| {
            anyhow::anyhow!(
                "login succeeded but no authSid in response\n\nRaw response:\n{}",
                &body[..body.len().min(1000)]
            )
        })?;

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

/// Extract the text content of a simple XML tag, stripping any CDATA wrapper.
///
/// Handles both `<tag>value</tag>` and `<tag><![CDATA[value]]></tag>`.
pub fn extract_xml_value(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&close)?;
    let raw = &body[start..start + end];
    let value = raw
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(raw);
    Some(value.to_string())
}
