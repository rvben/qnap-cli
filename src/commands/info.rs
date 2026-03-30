use anyhow::Result;

use crate::client::{QnapClient, format_uptime, xml_fields_to_map};
use crate::output::print_kv;

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
        return None; // version not available, skip
    }
    if major < 4 || (major == 4 && minor < 3) {
        return Some(format!(
            "firmware {} is older than QTS 4.3 — some commands may not work correctly\n  Run `qnap dump ./debug/` and open a GitHub issue if you encounter problems.",
            version
        ));
    }
    None
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    let fields: &[(&str, &str)] = &[
        ("model", "modelName"),
        ("hostname", "hostname"),
        ("serial", "serial_number"),
        ("firmware", "version"),
        ("build", "build"),
        ("timezone", "timezone"),
    ];

    let mut map = xml_fields_to_map(&body, fields);

    if let Some(uptime) = format_uptime(&body) {
        map.insert("uptime".to_string(), serde_json::Value::String(uptime));
    }

    if !json {
        if let Some(note) =
            firmware_compat_note(map.get("firmware").and_then(|v| v.as_str()).unwrap_or(""))
        {
            eprintln!("Note: {}", note);
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&map).unwrap_or_default());
        return Ok(());
    }

    let pairs: Vec<(String, String)> = map
        .into_iter()
        .map(|(k, v)| {
            let val = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            (k, val)
        })
        .collect();

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
        use crate::client::extract_xml_value;
        let version = extract_xml_value(SYSINFO, "version").unwrap_or_default();
        assert!(
            firmware_compat_note(&version).is_none(),
            "fixture firmware {} triggered compat warning",
            version
        );
    }
}
