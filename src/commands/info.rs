use anyhow::Result;
use serde::Serialize;

use crate::client::{QnapClient, Uptime, parse_uptime, parse_xml, xml_fields_to_map};
use crate::output::print_kv;

#[derive(Debug, Serialize)]
struct UptimeOutput {
    display: String,
    days: u64,
    hours: u64,
    minutes: u64,
    seconds: u64,
    total_seconds: u64,
}

#[derive(Debug, Serialize)]
struct InfoOutput {
    model: Option<String>,
    hostname: Option<String>,
    serial: Option<String>,
    firmware: Option<String>,
    build: Option<String>,
    timezone: Option<String>,
    uptime: Option<UptimeOutput>,
}

impl From<Uptime> for UptimeOutput {
    fn from(uptime: Uptime) -> Self {
        Self {
            display: uptime.display(),
            days: uptime.days,
            hours: uptime.hours,
            minutes: uptime.minutes,
            seconds: uptime.seconds,
            total_seconds: uptime.total_seconds(),
        }
    }
}

/// Returns a note when the firmware version is outside the tested range.
fn firmware_compat_note(version: &str) -> Option<String> {
    let parts: Vec<u32> = version
        .split('.')
        .take(2)
        .filter_map(|s| s.parse().ok())
        .collect();
    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    if major == 0 {
        return None;
    }
    if major < 4 || (major == 4 && minor < 3) {
        return Some(format!(
            "firmware {} is older than QTS 4.3 — some commands may not work correctly\n  Run `qnap dump ./debug/` and open a GitHub issue if you encounter problems.",
            version
        ));
    }
    None
}

fn build_info(body: &str) -> Result<InfoOutput> {
    let doc = parse_xml(body)?;
    let map = xml_fields_to_map(
        &doc,
        &[
            ("model", "modelName"),
            ("hostname", "hostname"),
            ("serial", "serial_number"),
            ("firmware", "version"),
            ("build", "build"),
            ("timezone", "timezone"),
        ],
    );

    Ok(InfoOutput {
        model: map
            .get("model")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        hostname: map
            .get("hostname")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        serial: map
            .get("serial")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        firmware: map
            .get("firmware")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        build: map
            .get("build")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        timezone: map
            .get("timezone")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        uptime: parse_uptime(&doc).map(Into::into),
    })
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    let info = build_info(&body)?;

    if !json {
        if let Some(note) = info.firmware.as_deref().and_then(firmware_compat_note) {
            eprintln!("Note: {}", note);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
        return Ok(());
    }

    let mut pairs = Vec::new();
    if let Some(value) = info.model {
        pairs.push(("model".to_string(), value));
    }
    if let Some(value) = info.hostname {
        pairs.push(("hostname".to_string(), value));
    }
    if let Some(value) = info.serial {
        pairs.push(("serial".to_string(), value));
    }
    if let Some(value) = info.firmware {
        pairs.push(("firmware".to_string(), value));
    }
    if let Some(value) = info.build {
        pairs.push(("build".to_string(), value));
    }
    if let Some(value) = info.timezone {
        pairs.push(("timezone".to_string(), value));
    }
    if let Some(value) = info.uptime {
        pairs.push(("uptime".to_string(), value.display));
    }

    print_kv(&pairs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSINFO: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/sysinfo.xml");

    #[test]
    fn test_firmware_compat_note_modern() {
        assert!(firmware_compat_note("5.2.6").is_none());
        assert!(firmware_compat_note("4.3.0").is_none());
        assert!(firmware_compat_note("4.5.0").is_none());
    }

    #[test]
    fn test_firmware_compat_note_old() {
        assert!(firmware_compat_note("4.2.6").is_some());
        assert!(firmware_compat_note("3.8.0").is_some());
    }

    #[test]
    fn test_firmware_compat_note_empty() {
        assert!(firmware_compat_note("").is_none());
    }

    #[test]
    fn fixture_sysinfo_firmware_is_supported() {
        let info = build_info(SYSINFO).unwrap();
        let version = info.firmware.unwrap_or_default();
        assert!(
            firmware_compat_note(&version).is_none(),
            "fixture firmware {} triggered compat warning",
            version
        );
    }
}
