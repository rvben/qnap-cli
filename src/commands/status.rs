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
        ("cpu_usage", "cpu_usage"),
        ("mem_total_mb", "total_memory"),
        ("mem_free_mb", "free_memory"),
        ("sys_temp_c", "sys_tempc"),
    ];

    let mut map = serde_json::Map::new();
    for (key, tag) in fields {
        if let Some(val) = extract_xml_value(&body, tag) {
            map.insert(key.to_string(), Value::String(val));
        }
    }

    // Compute used memory
    if let (Some(total), Some(free)) = (
        extract_xml_value(&body, "total_memory")
            .and_then(|s| s.trim().parse::<f64>().ok()),
        extract_xml_value(&body, "free_memory")
            .and_then(|s| s.trim().parse::<f64>().ok()),
    ) {
        map.insert(
            "mem_used_mb".to_string(),
            Value::String(format!("{:.1}", total - free)),
        );
    }

    // Uptime
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
