use anyhow::Result;
use serde_json::Value;

use crate::client::{extract_xml_value, QnapClient};
use crate::output::print_value;

// QNAP vol_status codes from QTS firmware source
fn vol_status_label(code: &str) -> &'static str {
    match code.trim() {
        "0" => "ready",
        "-1" => "not ready",
        "-4" => "error",
        "-5" => "degraded",
        "-6" => "not active",
        "-7" => "ready",
        _ => "unknown",
    }
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    // Volume list from disk manager
    let vol_body = client
        .get_cgi(
            "/cgi-bin/disk/disk_manage.cgi",
            &[("func", "extra_get"), ("extra_vol_index", "1")],
        )
        .await?;

    // Disk hardware info from sysinfo
    let sys_body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    // Parse volumes from repeated <row> blocks
    let mut volumes: Vec<Value> = Vec::new();
    let mut remaining = vol_body.as_str();
    while let Some(start) = remaining.find("<row>") {
        remaining = &remaining[start + "<row>".len()..];
        let end = remaining.find("</row>").unwrap_or(remaining.len());
        let block = &remaining[..end];
        remaining = &remaining[end..];

        let status_code = extract_xml_value(block, "vol_status").unwrap_or_default();
        let mut map = serde_json::Map::new();
        map.insert(
            "label".into(),
            Value::String(
                extract_xml_value(block, "vol_label").unwrap_or_default(),
            ),
        );
        map.insert(
            "status".into(),
            Value::String(vol_status_label(&status_code).to_string()),
        );
        map.insert(
            "pool".into(),
            Value::String(
                extract_xml_value(block, "poolID").unwrap_or_default(),
            ),
        );
        map.insert(
            "type".into(),
            Value::String(
                extract_xml_value(block, "lv_type").unwrap_or_default(),
            ),
        );
        volumes.push(Value::Object(map));
    }

    // Parse disk hardware from sysinfo
    let disk_count: usize = extract_xml_value(&sys_body, "disk_num")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let mut disks: Vec<Value> = Vec::new();
    for i in 1..=disk_count {
        let installed = extract_xml_value(&sys_body, &format!("disk_installed{}", i))
            .unwrap_or_default();
        if installed != "1" {
            continue;
        }
        let alias = extract_xml_value(&sys_body, &format!("hd_pd_alias{}", i))
            .unwrap_or_default();
        let temp = extract_xml_value(&sys_body, &format!("tempc{}", i))
            .unwrap_or_default();
        let is_ssd = extract_xml_value(&sys_body, &format!("hd_is_ssd{}", i))
            .unwrap_or_default();

        let mut map = serde_json::Map::new();
        map.insert("slot".into(), Value::Number(i.into()));
        map.insert("type".into(), Value::String(alias));
        map.insert(
            "kind".into(),
            Value::String(if is_ssd == "1" { "SSD" } else { "HDD" }.to_string()),
        );
        if !temp.is_empty() && temp != "0" {
            map.insert("temp_c".into(), Value::String(temp));
        }
        disks.push(Value::Object(map));
    }

    let output = serde_json::json!({
        "volumes": volumes,
        "disks": disks,
    });

    print_value(&output, json);
    Ok(())
}
