use anyhow::{Context, Result, bail};
use reqwest::{Client, ClientBuilder, Response};
use roxmltree::{Document, Node};
use serde::Serialize;
use std::time::Duration;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Uptime {
    pub days: u64,
    pub hours: u64,
    pub minutes: u64,
    pub seconds: u64,
}

impl Uptime {
    pub fn display(&self) -> String {
        format!("{}d {}h {}m", self.days, self.hours, self.minutes)
    }

    pub fn total_seconds(&self) -> u64 {
        self.days * 24 * 60 * 60 + self.hours * 60 * 60 + self.minutes * 60 + self.seconds
    }
}

pub struct QnapClient {
    http: Client,
    base_url: String,
    sid: Option<String>,
}

impl QnapClient {
    pub fn new(config: &Config) -> Result<Self> {
        Self::build(config.host()?, config.insecure()?)
    }

    /// Authenticate with the NAS and cache the returned session ID.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        use base64::{Engine, engine::general_purpose::STANDARD};

        let pwd = STANDARD.encode(password.as_bytes());
        let url = format!("{}/cgi-bin/authLogin.cgi", self.base_url);

        let resp = self
            .send_checked(
                self.http.post(&url).form(&[
                    ("user", username),
                    ("pwd", pwd.as_str()),
                    ("serviceKey", "1"),
                    ("client_app", "qnap-cli"),
                ]),
                "authentication request",
            )
            .await?;

        let body = resp
            .text()
            .await
            .context("failed to read authentication response")?;
        let doc = parse_xml(&body)?;

        let passed = xml_value(&doc, "authPassed").unwrap_or_default();
        if passed != "1" {
            let err = xml_value(&doc, "errorValue").unwrap_or_default();
            bail!("authentication failed (errorValue={})", err);
        }

        let sid = xml_value(&doc, "authSid").ok_or_else(|| {
            anyhow::anyhow!(
                "login succeeded but no authSid in response\n\nRaw response:\n{}",
                snippet(&body)
            )
        })?;

        self.sid = Some(sid);
        Ok(())
    }

    pub async fn get_cgi(&self, path: &str, params: &[(&str, &str)]) -> Result<String> {
        let sid = self
            .sid
            .as_deref()
            .context("client is not authenticated; call login first")?;
        let mut all_params: Vec<(&str, &str)> = params.to_vec();
        all_params.push(("sid", sid));

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .send_checked(
                self.http.get(&url).query(&all_params),
                &format!("request to {}", path),
            )
            .await?;

        resp.text().await.context("failed to read response body")
    }

    /// Upload a file to a remote directory via multipart POST.
    pub async fn upload_file(
        &self,
        dest_path: &str,
        filename: &str,
        data: Vec<u8>,
        overwrite: bool,
    ) -> Result<serde_json::Value> {
        let sid = self
            .sid
            .as_deref()
            .context("client is not authenticated; call login first")?;

        let url = format!(
            "{}/cgi-bin/filemanager/utilRequest.cgi",
            self.base_url
        );

        let file_part = reqwest::multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")
            .context("invalid MIME type")?;

        let form = reqwest::multipart::Form::new()
            .text("func", "upload")
            .text("type", "1")
            .text("dest_path", dest_path.to_string())
            .text("progress_id", "1")
            .text("overwrite", if overwrite { "1" } else { "0" })
            .text("sid", sid.to_string())
            .part("file", file_part);

        let resp = self
            .send_checked(self.http.post(&url).multipart(form), "file upload")
            .await?;
        let body = resp.text().await.context("failed to read upload response")?;
        serde_json::from_str(&body)
            .with_context(|| format!("failed to parse upload response: {}", snippet(&body)))
    }

    /// Start a file download, returning the raw HTTP response for streaming.
    pub async fn get_file_response(
        &self,
        source_path: &str,
        source_file: &str,
    ) -> Result<reqwest::Response> {
        let sid = self
            .sid
            .as_deref()
            .context("client is not authenticated; call login first")?;

        let url = format!(
            "{}/cgi-bin/filemanager/utilRequest.cgi",
            self.base_url
        );

        let resp = self
            .http
            .get(&url)
            .query(&[
                ("func", "download"),
                ("source_path", source_path),
                ("source_file", source_file),
                ("sid", sid),
            ])
            .send()
            .await
            .map_err(map_transport_error)?;

        if !resp.status().is_success() {
            bail!("download failed with HTTP {}", resp.status());
        }
        Ok(resp)
    }

    pub async fn get_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let sid = self
            .sid
            .as_deref()
            .context("client is not authenticated; call login first")?;
        let mut all_params: Vec<(&str, &str)> = params.to_vec();
        all_params.push(("sid", sid));

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .send_checked(
                self.http.get(&url).query(&all_params),
                &format!("request to {}", path),
            )
            .await?;

        let body = resp.text().await.context("failed to read JSON response")?;
        serde_json::from_str::<T>(&body).with_context(|| {
            format!(
                "failed to parse JSON response from {}: {}",
                path,
                snippet(&body)
            )
        })
    }

    async fn send_checked(
        &self,
        request: reqwest::RequestBuilder,
        context_label: &str,
    ) -> Result<Response> {
        let resp = request.send().await.map_err(map_transport_error)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }

        let body = resp.text().await.unwrap_or_default();
        bail!(
            "{} failed with HTTP {}: {}",
            context_label,
            status,
            snippet(&body)
        );
    }

    fn build(base_url: String, insecure: bool) -> Result<Self> {
        let http = ClientBuilder::new()
            .danger_accept_invalid_certs(insecure)
            .cookie_store(true)
            .timeout(Duration::from_secs(300))
            .build()?;

        Ok(Self {
            http,
            base_url,
            sid: None,
        })
    }

    #[cfg(test)]
    fn new_for_test(base_url: String) -> Result<Self> {
        Self::build(base_url.trim_end_matches('/').to_string(), false)
    }
}

fn map_transport_error(err: reqwest::Error) -> anyhow::Error {
    let message = err.to_string();
    if message.contains("certificate") || message.contains("validity") {
        anyhow::anyhow!(
            "TLS certificate error — retry with `--insecure` to skip verification\n  ({})",
            err
        )
    } else {
        anyhow::anyhow!("failed to reach NAS: {}", err)
    }
}

fn snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(empty response body)".to_string();
    }

    let mut value = trimmed.replace('\n', " ");
    if value.len() > 400 {
        value.truncate(400);
        value.push_str("...");
    }
    value
}

pub fn parse_xml(body: &str) -> Result<Document<'_>> {
    Document::parse(body).context("failed to parse XML response")
}

pub fn xml_value(doc: &Document<'_>, tag: &str) -> Option<String> {
    doc.descendants()
        .find(|node| node.has_tag_name(tag))
        .map(node_text)
}

pub fn xml_value_in(node: Node<'_, '_>, tag: &str) -> Option<String> {
    node.descendants()
        .find(|child| child.has_tag_name(tag))
        .map(node_text)
}

fn node_text(node: Node<'_, '_>) -> String {
    node.text().unwrap_or_default().trim().to_string()
}

pub fn extract_xml_value(body: &str, tag: &str) -> Option<String> {
    let doc = parse_xml(body).ok()?;
    xml_value(&doc, tag)
}

pub fn xml_fields_to_map(
    doc: &Document<'_>,
    fields: &[(&str, &str)],
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (key, tag) in fields {
        if let Some(val) = xml_value(doc, tag) {
            map.insert(key.to_string(), serde_json::Value::String(val));
        }
    }
    map
}

pub fn parse_uptime(doc: &Document<'_>) -> Option<Uptime> {
    let days = xml_value(doc, "uptime_day")?.parse().ok()?;
    let hours = xml_value(doc, "uptime_hour")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let minutes = xml_value(doc, "uptime_min")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let seconds = xml_value(doc, "uptime_sec")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    Some(Uptime {
        days,
        hours,
        minutes,
        seconds,
    })
}

#[cfg(test)]
pub fn format_uptime(body: &str) -> Option<String> {
    let doc = parse_xml(body).ok()?;
    parse_uptime(&doc).map(|uptime| uptime.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
            extract_xml_value("<QDocRoot><foo>1</foo><foo>2</foo></QDocRoot>", "foo"),
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

    #[test]
    fn test_xml_fields_to_map_present_fields() {
        let body = "<hostname>MYNAS</hostname><version>5.1.0</version>";
        let xml = format!("<QDocRoot>{}</QDocRoot>", body);
        let doc = parse_xml(&xml).unwrap();
        let fields = &[("hostname", "hostname"), ("firmware", "version")];
        let map = xml_fields_to_map(&doc, fields);
        assert_eq!(map.get("hostname").and_then(|v| v.as_str()), Some("MYNAS"));
        assert_eq!(map.get("firmware").and_then(|v| v.as_str()), Some("5.1.0"));
    }

    #[test]
    fn test_xml_fields_to_map_missing_fields_are_skipped() {
        let doc = parse_xml("<QDocRoot><hostname>MYNAS</hostname></QDocRoot>").unwrap();
        let fields = &[("hostname", "hostname"), ("missing", "nothere")];
        let map = xml_fields_to_map(&doc, fields);
        assert!(map.contains_key("hostname"));
        assert!(!map.contains_key("missing"));
    }

    #[test]
    fn test_xml_fields_to_map_empty_body() {
        let doc = parse_xml("<QDocRoot/>").unwrap();
        let map = xml_fields_to_map(&doc, &[("key", "tag")]);
        assert!(map.is_empty());
    }

    #[test]
    fn test_format_uptime_all_fields() {
        let body = "<QDocRoot><uptime_day>5</uptime_day><uptime_hour>12</uptime_hour><uptime_min>30</uptime_min><uptime_sec>9</uptime_sec></QDocRoot>";
        assert_eq!(format_uptime(body), Some("5d 12h 30m".to_string()));
    }

    #[test]
    fn test_format_uptime_zero_day() {
        let body = "<QDocRoot><uptime_day>0</uptime_day><uptime_hour>0</uptime_hour><uptime_min>5</uptime_min></QDocRoot>";
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
                "<QDocRoot><uptime_day></uptime_day><uptime_hour>1</uptime_hour><uptime_min>0</uptime_min></QDocRoot>"
            ),
            None
        );
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

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        client.login("admin", "correct-password").await.unwrap();
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response("0", "", "6")))
            .mount(&server)
            .await;

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        let err = client.login("admin", "wrong-password").await.unwrap_err();
        assert!(err.to_string().contains("authentication failed"));
        assert!(err.to_string().contains("errorValue=6"));
    }

    #[tokio::test]
    async fn test_login_missing_sid_in_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<QDocRoot><authPassed>1</authPassed></QDocRoot>"),
            )
            .mount(&server)
            .await;

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        let err = client.login("admin", "pass").await.unwrap_err();
        assert!(err.to_string().contains("no authSid"));
    }

    #[tokio::test]
    async fn test_login_sends_base64_encoded_password() {
        use base64::{Engine, engine::general_purpose::STANDARD};

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

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        client.login("admin", "my-secret").await.unwrap();
    }

    #[tokio::test]
    async fn test_get_cgi_appends_sid() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response(
                "1",
                "test-sid-123",
                "",
            )))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/cgi-bin/management/manaRequest.cgi"))
            .and(query_param("sid", "test-sid-123"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<QDocRoot><ok/></QDocRoot>"))
            .expect(1)
            .mount(&server)
            .await;

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        client.login("admin", "pass").await.unwrap();
        client
            .get_cgi(
                "/cgi-bin/management/manaRequest.cgi",
                &[("subfunc", "sysinfo")],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_get_cgi_surfaces_http_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cgi-bin/authLogin.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(auth_response(
                "1",
                "test-sid-123",
                "",
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cgi-bin/management/manaRequest.cgi"))
            .respond_with(ResponseTemplate::new(500).set_body_string("bad gateway"))
            .mount(&server)
            .await;

        let mut client = QnapClient::new_for_test(server.uri()).unwrap();
        client.login("admin", "pass").await.unwrap();
        let err = client
            .get_cgi(
                "/cgi-bin/management/manaRequest.cgi",
                &[("subfunc", "sysinfo")],
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTTP 500"));
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    const SYSINFO: &str = include_str!("../tests/fixtures/ts-xa28a-qts52/sysinfo.xml");
    const VOLUMES: &str = include_str!("../tests/fixtures/ts-xa28a-qts52/volumes.xml");

    #[test]
    fn fixture_sysinfo_required_fields_present() {
        let doc = parse_xml(SYSINFO).unwrap();
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
                xml_value(&doc, tag).is_some(),
                "missing sysinfo field: {}",
                tag
            );
        }
    }

    #[test]
    fn fixture_sysinfo_uptime_parses() {
        let doc = parse_xml(SYSINFO).unwrap();
        let uptime = parse_uptime(&doc).unwrap();
        assert!(uptime.hours < 24, "hours out of range: {}", uptime.hours);
        assert!(
            uptime.minutes < 60,
            "minutes out of range: {}",
            uptime.minutes
        );
        assert!(
            uptime.seconds < 60,
            "seconds out of range: {}",
            uptime.seconds
        );
    }

    #[test]
    fn fixture_sysinfo_xml_fields_to_map() {
        let doc = parse_xml(SYSINFO).unwrap();
        let fields = &[
            ("model", "modelName"),
            ("firmware", "version"),
            ("hostname", "hostname"),
        ];
        let map = xml_fields_to_map(&doc, fields);
        assert_eq!(map.len(), 3, "expected 3 fields from sysinfo");
        assert!(!map["model"].as_str().unwrap().is_empty());
        assert!(!map["firmware"].as_str().unwrap().is_empty());
    }

    #[test]
    fn fixture_volumes_contains_rows() {
        let doc = parse_xml(VOLUMES).unwrap();
        assert!(
            doc.descendants().any(|node| node.has_tag_name("row")),
            "no <row> elements found in volumes fixture"
        );
    }

    #[test]
    fn fixture_volumes_row_fields_present() {
        let doc = parse_xml(VOLUMES).unwrap();
        let rows: Vec<_> = doc
            .descendants()
            .filter(|node| node.has_tag_name("row"))
            .collect();
        assert!(
            !rows.is_empty(),
            "no <row> elements parsed from volumes fixture"
        );

        for (index, row) in rows.iter().enumerate() {
            assert!(
                xml_value_in(*row, "vol_status").is_some(),
                "row {} missing vol_status",
                index + 1
            );
            assert!(
                xml_value_in(*row, "vol_label").is_some(),
                "row {} missing vol_label",
                index + 1
            );
        }
    }
}
