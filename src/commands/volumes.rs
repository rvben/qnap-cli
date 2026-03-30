use anyhow::Result;
use serde_json::Value;

use crate::client::{extract_xml_value, QnapClient};
use crate::output::print_value;

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "volumeInfo")],
        )
        .await?;

    // Parse repeated <volume> blocks
    let mut volumes: Vec<Value> = Vec::new();
    let mut remaining = body.as_str();

    while let Some(start) = remaining.find("<volume>") {
        remaining = &remaining[start + "<volume>".len()..];
        let end = remaining.find("</volume>").unwrap_or(remaining.len());
        let block = &remaining[..end];
        remaining = &remaining[end..];

        let fields: &[(&str, &str)] = &[
            ("id", "volumeID"),
            ("label", "label"),
            ("status", "status"),
            ("total_gb", "totalSize"),
            ("used_gb", "usedSize"),
            ("free_gb", "freeSize"),
            ("raid", "raidType"),
            ("fs", "fileSystem"),
        ];

        let mut map = serde_json::Map::new();
        for (key, tag) in fields {
            if let Some(val) = extract_xml_value(block, tag) {
                map.insert(key.to_string(), Value::String(val));
            }
        }

        if !map.is_empty() {
            volumes.push(Value::Object(map));
        }
    }

    if volumes.is_empty() {
        // Fallback: try disk info
        let body2 = client
            .get_cgi(
                "/cgi-bin/management/manaRequest.cgi",
                &[("subfunc", "disk_overview")],
            )
            .await?;
        println!("[raw response]\n{}", &body2[..body2.len().min(2000)]);
        return Ok(());
    }

    print_value(&Value::Array(volumes), json);
    Ok(())
}
