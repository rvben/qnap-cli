use anyhow::{Context, Result, bail};
use reqwest::{Client, ClientBuilder};
use std::time::Duration;

use crate::config::Config;

pub struct QnapClient {
    http: Client,
    base_url: String,
    sid: Option<String>,
}

impl QnapClient {
    pub fn new(config: &Config) -> Result<Self> {
        let host = config.host()?;
        let insecure = config.insecure();

        let http = ClientBuilder::new()
            .danger_accept_invalid_certs(insecure)
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
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

    /// Authenticate with the NAS. Returns the session ID prefix for display purposes.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<String> {
        use base64::{Engine, engine::general_purpose::STANDARD};
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
        let mut all_params: Vec<(&str, &str)> = params.to_vec();
        all_params.push(("sid", sid));

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .query(&all_params)
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
        let mut all_params: Vec<(&str, &str)> = params.to_vec();
        all_params.push(("sid", sid));

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .query(&all_params)
            .send()
            .await
            .context("request failed")?;

        resp.json::<T>()
            .await
            .context("failed to parse JSON response")
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

/// Build a serde_json map from XML body and a list of (output_key, xml_tag) pairs.
pub fn xml_fields_to_map(
    body: &str,
    fields: &[(&str, &str)],
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (key, tag) in fields {
        if let Some(val) = extract_xml_value(body, tag) {
            map.insert(key.to_string(), serde_json::Value::String(val));
        }
    }
    map
}

/// Format uptime from the three separate XML fields in sysinfo.
pub fn format_uptime(body: &str) -> Option<String> {
    let day = extract_xml_value(body, "uptime_day")?;
    if day.is_empty() {
        return None;
    }
    let hour = extract_xml_value(body, "uptime_hour").unwrap_or_default();
    let min = extract_xml_value(body, "uptime_min").unwrap_or_default();
    Some(format!("{}d {}h {}m", day, hour, min))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── extract_xml_value ────────────────────────────────────────────────────

    #[test]
    fn test_extract_xml_value_plain() {
        assert_eq!(
            extract_xml_value("<hostname>MYNAS</hostname>", "hostname"),
            Some("MYNAS".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value_cdata() {
        assert_eq!(
            extract_xml_value("<cpu_usage><![CDATA[12 %]]></cpu_usage>", "cpu_usage"),
            Some("12 %".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value_numeric_cdata() {
        assert_eq!(
            extract_xml_value("<uptime_day><![CDATA[5]]></uptime_day>", "uptime_day"),
            Some("5".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value_missing_tag() {
        assert_eq!(extract_xml_value("<foo>bar</foo>", "baz"), None);
    }

    #[test]
    fn test_extract_xml_value_empty_value() {
        assert_eq!(
            extract_xml_value("<foo></foo>", "foo"),
            Some("".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value_returns_first_match() {
        assert_eq!(
            extract_xml_value("<foo>1</foo><foo>2</foo>", "foo"),
            Some("1".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value_realistic_sysinfo() {
        let body = r#"<QDocRoot>
            <modelName><![CDATA[TS-453Be]]></modelName>
            <hostname><![CDATA[MYNAS]]></hostname>
            <version><![CDATA[5.1.0.2348]]></version>
        </QDocRoot>"#;
        assert_eq!(
            extract_xml_value(body, "modelName"),
            Some("TS-453Be".to_string())
        );
        assert_eq!(
            extract_xml_value(body, "hostname"),
            Some("MYNAS".to_string())
        );
        assert_eq!(extract_xml_value(body, "missing"), None);
    }

    // ── xml_fields_to_map ────────────────────────────────────────────────────

    #[test]
    fn test_xml_fields_to_map_present_fields() {
        let body = "<hostname>MYNAS</hostname><version>5.1.0</version>";
        let fields = &[("hostname", "hostname"), ("firmware", "version")];
        let map = xml_fields_to_map(body, fields);
        assert_eq!(map.get("hostname").and_then(|v| v.as_str()), Some("MYNAS"));
        assert_eq!(map.get("firmware").and_then(|v| v.as_str()), Some("5.1.0"));
    }

    #[test]
    fn test_xml_fields_to_map_missing_fields_are_skipped() {
        let body = "<hostname>MYNAS</hostname>";
        let fields = &[("hostname", "hostname"), ("missing", "nothere")];
        let map = xml_fields_to_map(body, fields);
        assert!(map.contains_key("hostname"));
        assert!(!map.contains_key("missing"));
    }

    #[test]
    fn test_xml_fields_to_map_empty_body() {
        let map = xml_fields_to_map("", &[("key", "tag")]);
        assert!(map.is_empty());
    }

    // ── format_uptime ────────────────────────────────────────────────────────

    #[test]
    fn test_format_uptime_all_fields() {
        let body =
            "<uptime_day>5</uptime_day><uptime_hour>12</uptime_hour><uptime_min>30</uptime_min>";
        assert_eq!(format_uptime(body), Some("5d 12h 30m".to_string()));
    }

    #[test]
    fn test_format_uptime_zero_day() {
        let body =
            "<uptime_day>0</uptime_day><uptime_hour>0</uptime_hour><uptime_min>5</uptime_min>";
        assert_eq!(format_uptime(body), Some("0d 0h 5m".to_string()));
    }

    #[test]
    fn test_format_uptime_missing_day_tag() {
        assert_eq!(format_uptime("<foo>bar</foo>"), None);
    }

    #[test]
    fn test_format_uptime_empty_day_value() {
        assert_eq!(
            format_uptime(
                "<uptime_day></uptime_day><uptime_hour>1</uptime_hour><uptime_min>0</uptime_min>"
            ),
            None
        );
    }

    // ── QnapClient::login (mock HTTP) ─────────────────────────────────────────

    fn test_config(host: String) -> Config {
        Config {
            host: Some(host),
            username: Some("admin".to_string()),
            insecure: Some(false),
        }
    }

    fn auth_response(passed: &str, sid: &str, error: &str) -> String {
        format!(
            "<QDocRoot><authPassed><![CDATA[{}]]></authPassed><authSid><![CDATA[{}]]></authSid><errorValue><![CDATA[{}]]></errorValue></QDocRoot>",
            passed, sid, error
        )
    }

    #[tokio::test]
    async fn test_login_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response(
                "1",
                "abc123def456",
                "",
            )))
            .mount(&server)
            .await;

        let config = test_config(server.uri());
        let mut client = QnapClient::new(&config).unwrap();
        let sid = client.login("admin", "correct-password").await.unwrap();
        assert_eq!(sid, "abc123def456");
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response("0", "", "6")))
            .mount(&server)
            .await;

        let config = test_config(server.uri());
        let mut client = QnapClient::new(&config).unwrap();
        let err = client.login("admin", "wrong-password").await.unwrap_err();
        assert!(
            err.to_string().contains("authentication failed"),
            "unexpected error: {}",
            err
        );
        assert!(err.to_string().contains("errorValue=6"));
    }

    #[tokio::test]
    async fn test_login_missing_sid_in_response() {
        let server = MockServer::start().await;
        // authPassed=1 but no authSid
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<QDocRoot><authPassed>1</authPassed></QDocRoot>"),
            )
            .mount(&server)
            .await;

        let config = test_config(server.uri());
        let mut client = QnapClient::new(&config).unwrap();
        let err = client.login("admin", "pass").await.unwrap_err();
        assert!(
            err.to_string().contains("no authSid"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_login_sends_base64_encoded_password() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use wiremock::matchers::body_string_contains;

        let server = MockServer::start().await;
        let expected_pwd = STANDARD.encode("my-secret".as_bytes());

        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .and(body_string_contains(format!("pwd={}", expected_pwd)))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(auth_response("1", "sid-xyz", "")),
            )
            .expect(1)
            .mount(&server)
            .await;

        let config = test_config(server.uri());
        let mut client = QnapClient::new(&config).unwrap();
        client.login("admin", "my-secret").await.unwrap();
    }

    #[tokio::test]
    async fn test_get_cgi_appends_sid() {
        use wiremock::matchers::query_param;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cgi-bin/management/manaRequest.cgi"))
            .and(query_param("sid", "test-sid-123"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<ok/>"))
            .expect(1)
            .mount(&server)
            .await;

        // Directly set sid via login mock
        let login_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response(
                "1",
                "test-sid-123",
                "",
            )))
            .mount(&login_server)
            .await;

        let config = test_config(login_server.uri());
        let mut client = QnapClient::new(&config).unwrap();
        client.login("admin", "pass").await.unwrap();

        // Now make a CGI request to a different server that expects the SID
        // We rebuild the client pointing at the second server to check the param
        let config2 = test_config(server.uri());
        let mut client2 = QnapClient::new(&config2).unwrap();
        // Manually set sid by going through login first (we need a way to set it)
        // Instead, test via a fresh mock that verifies the sid query param
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response(
                "1",
                "test-sid-123",
                "",
            )))
            .mount(&server)
            .await;
        client2.login("admin", "pass").await.unwrap();
        client2
            .get_cgi(
                "/cgi-bin/management/manaRequest.cgi",
                &[("subfunc", "sysinfo")],
            )
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    const SYSINFO: &str = include_str!("../tests/fixtures/ts-xa28a-qts52/sysinfo.xml");
    const VOLUMES: &str = include_str!("../tests/fixtures/ts-xa28a-qts52/volumes.xml");

    #[test]
    fn fixture_sysinfo_required_fields_present() {
        for tag in &[
            "modelName",
            "hostname",
            "version",
            "build",
            "total_memory",
            "uptime_day",
            "uptime_hour",
            "uptime_min",
        ] {
            assert!(
                extract_xml_value(SYSINFO, tag).is_some(),
                "missing sysinfo field: {}",
                tag
            );
        }
    }

    #[test]
    fn fixture_sysinfo_uptime_parses() {
        let uptime = format_uptime(SYSINFO);
        assert!(uptime.is_some(), "format_uptime returned None for fixture");
        let s = uptime.unwrap();
        assert!(
            s.contains('d') && s.contains('h') && s.contains('m'),
            "unexpected uptime format: {}",
            s
        );
    }

    #[test]
    fn fixture_sysinfo_xml_fields_to_map() {
        let fields = &[
            ("model", "modelName"),
            ("firmware", "version"),
            ("hostname", "hostname"),
        ];
        let map = xml_fields_to_map(SYSINFO, fields);
        assert_eq!(map.len(), 3, "expected 3 fields from sysinfo");
        assert!(map["model"].as_str().unwrap().len() > 0);
        assert!(map["firmware"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn fixture_volumes_contains_rows() {
        assert!(
            VOLUMES.contains("<row>"),
            "no <row> elements found in volumes fixture"
        );
    }

    #[test]
    fn fixture_volumes_row_fields_present() {
        let mut remaining = VOLUMES;
        let mut count = 0;
        while let Some(start) = remaining.find("<row>") {
            remaining = &remaining[start + "<row>".len()..];
            let end = remaining.find("</row>").unwrap_or(remaining.len());
            let block = &remaining[..end];
            remaining = &remaining[end..];
            count += 1;

            assert!(
                extract_xml_value(block, "vol_status").is_some(),
                "row {} missing vol_status",
                count
            );
            assert!(
                extract_xml_value(block, "vol_label").is_some(),
                "row {} missing vol_label",
                count
            );
        }
        assert!(count > 0, "no <row> elements parsed from volumes fixture");
    }
}
