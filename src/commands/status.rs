use anyhow::Result;

use crate::client::{QnapClient, extract_xml_value, format_uptime};
use crate::output::{fmt_temp, print_kv};

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    let mut pairs: Vec<(String, String)> = Vec::new();

    if let Some(cpu) = extract_xml_value(&body, "cpu_usage") {
        // The QNAP sysinfo API already includes the % sign in the value
        pairs.push(("cpu_usage".into(), cpu.trim().to_string()));
    }

    let total = extract_xml_value(&body, "total_memory").and_then(|s| s.trim().parse::<f64>().ok());
    let free = extract_xml_value(&body, "free_memory").and_then(|s| s.trim().parse::<f64>().ok());

    if let Some(t) = total {
        pairs.push(("mem_total_mb".into(), format!("{:.0}", t)));
    }
    if let (Some(t), Some(f)) = (total, free) {
        pairs.push(("mem_used_mb".into(), format!("{:.0}", t - f)));
        pairs.push(("mem_free_mb".into(), format!("{:.0}", f)));
    }

    let temp_raw = extract_xml_value(&body, "sys_tempc").unwrap_or_default();
    if !temp_raw.is_empty() {
        let display = if json {
            format!("{}°C", temp_raw.trim())
        } else {
            fmt_temp(&temp_raw)
        };
        pairs.push(("temp".into(), display));
    }

    if let Some(uptime) = format_uptime(&body) {
        pairs.push(("uptime".into(), uptime));
    }

    if json {
        let map: serde_json::Map<String, serde_json::Value> = pairs
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();
        println!("{}", serde_json::to_string_pretty(&map).unwrap_or_default());
        return Ok(());
    }

    print_kv(&pairs);
    Ok(())
}
