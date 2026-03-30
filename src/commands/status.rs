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
        ("cpu_usage", "cpuUsage"),
        ("mem_total_mb", "totalRam"),
        ("mem_used_mb", "usedRam"),
        ("mem_free_mb", "freeRam"),
        ("uptime", "uptime"),
        ("cpu_temp_c", "CPUTemperature"),
        ("sys_temp_c", "SystemTemperature"),
    ];

    let mut map = serde_json::Map::new();
    for (key, tag) in fields {
        if let Some(val) = extract_xml_value(&body, tag) {
            map.insert(key.to_string(), Value::String(val));
        }
    }

    print_value(&Value::Object(map), json);
    Ok(())
}
