use anyhow::Result;
use serde_json::Value;

use crate::client::{extract_xml_value, QnapClient};
use crate::output::print_value;

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

    let mut map = serde_json::Map::new();
    for (key, tag) in fields {
        if let Some(val) = extract_xml_value(&body, tag) {
            map.insert(key.to_string(), Value::String(val));
        }
    }

    // Compose uptime from separate day/hour/min fields
    let day = extract_xml_value(&body, "uptime_day").unwrap_or_default();
    let hour = extract_xml_value(&body, "uptime_hour").unwrap_or_default();
    let min = extract_xml_value(&body, "uptime_min").unwrap_or_default();
    if !day.is_empty() {
        map.insert(
            "uptime".to_string(),
            Value::String(format!("{}d {}h {}m", day, hour, min)),
        );
    }

    print_value(&Value::Object(map), json);
    Ok(())
}
