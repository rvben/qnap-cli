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
        ("firmware", "version"),
        ("build", "build"),
        ("serial", "serialNumber"),
        ("hostname", "hostName"),
        ("uptime", "uptime"),
        ("timezone", "timeZone"),
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
