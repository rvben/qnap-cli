use anyhow::{Context, Result};
use std::path::Path;

use crate::client::{QnapClient, extract_xml_value};

/// Replace the content between an XML tag pair with a fixed value.
///
/// Handles both plain (`<tag>value</tag>`) and CDATA
/// (`<tag><![CDATA[value]]></tag>`) forms by replacing everything
/// between the opening and closing tag.
fn redact_xml_tag(body: &str, tag: &str, replacement: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut result = String::with_capacity(body.len());
    let mut remaining = body;
    while let Some(start) = remaining.find(&open) {
        result.push_str(&remaining[..start + open.len()]);
        remaining = &remaining[start + open.len()..];
        if let Some(end) = remaining.find(&close) {
            result.push_str(replacement);
            remaining = &remaining[end..];
        }
    }
    result.push_str(remaining);
    result
}

fn anonymize_xml(body: &str) -> String {
    let body = redact_xml_tag(body, "serial_number", "REDACTED");
    // Hostname appears under two tag names depending on firmware version
    let body = redact_xml_tag(&body, "hostname", "<![CDATA[REDACTED]]>");
    let body = redact_xml_tag(&body, "server_name", "<![CDATA[REDACTED]]>");
    // Network interface IPs (eth_ip1 … eth_ip4)
    let body = (1..=4).fold(body, |b, i| {
        redact_xml_tag(&b, &format!("eth_ip{}", i), "x.x.x.x")
    });
    // DNS entries — plain tags and CDATA list form
    let body = redact_xml_tag(&body, "dns1", "x.x.x.x");
    let body = redact_xml_tag(&body, "dns2", "x.x.x.x");
    redact_xml_tag(&body, "DNS_LIST", "<![CDATA[x.x.x.x]]>")
}

/// Replace non-generic owner names in a file-list JSON response.
///
/// Keeps `admin`, `administrators`, and `everyone` as-is since those
/// are built-in QNAP names that carry no personal information.
fn anonymize_json_owners(body: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    let generic = ["admin", "administrators", "everyone", ""];
    if let Some(entries) = v.get_mut("datas").and_then(|d| d.as_array_mut()) {
        for entry in entries {
            for field in ["owner", "group"] {
                if let Some(val) = entry.get_mut(field) {
                    if val.as_str().map(|s| !generic.contains(&s)).unwrap_or(false) {
                        *val = serde_json::Value::String("user".to_string());
                    }
                }
            }
        }
    }
    serde_json::to_string(&v).unwrap_or_else(|_| body.to_string())
}

pub async fn run(client: &QnapClient, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("failed to create directory {}", dir.display()))?;

    let mut saved = Vec::new();

    macro_rules! save {
        ($filename:expr, $body:expr) => {
            let path = dir.join($filename);
            std::fs::write(&path, $body)
                .with_context(|| format!("failed to write {}", path.display()))?;
            println!("  {}", $filename);
            saved.push($filename);
        };
    }

    println!("Fetching raw API responses...");

    let sysinfo = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;
    save!("sysinfo.xml", anonymize_xml(&sysinfo));

    let volumes = client
        .get_cgi(
            "/cgi-bin/disk/disk_manage.cgi",
            &[("func", "extra_get"), ("extra_vol_index", "1")],
        )
        .await?;
    save!("volumes.xml", anonymize_xml(&volumes));

    let shares = client
        .get_cgi(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "get_tree"), ("node", "share_root")],
        )
        .await?;
    save!("shares.json", &shares);

    // List the first available share (/ is not a valid path on QNAP)
    let first_share = serde_json::from_str::<serde_json::Value>(&shares)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .and_then(|arr| arr.into_iter().next())
        .and_then(|e| e.get("id").and_then(|id| id.as_str()).map(str::to_string))
        .unwrap_or_else(|| "/Public".to_string());

    let files_share = client
        .get_cgi(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[
                ("func", "get_list"),
                ("path", &first_share),
                ("limit", "50"),
                ("start", "0"),
                ("type", "both"),
                ("sortby", "filename"),
                ("order", "ASC"),
            ],
        )
        .await?;
    save!("files-share.json", anonymize_json_owners(&files_share));

    // Save a metadata summary so fixture contributors can identify their device
    let model = extract_xml_value(&sysinfo, "modelName").unwrap_or_else(|| "unknown".to_string());
    let firmware = extract_xml_value(&sysinfo, "version").unwrap_or_else(|| "unknown".to_string());
    let build = extract_xml_value(&sysinfo, "build").unwrap_or_else(|| "unknown".to_string());

    let meta = format!(
        "model = \"{}\"\nfirmware = \"{}\"\nbuild = \"{}\"\n",
        model, firmware, build
    );
    save!("meta.toml", &meta);

    println!("\nSaved {} files to {}", saved.len(), dir.display());
    println!(
        "  Model: {}  Firmware: {} (build {})",
        model, firmware, build
    );
    println!();
    println!("To help test compatibility with your device, open a GitHub issue at:");
    println!("  https://github.com/rvben/qnap-cli/issues");
    println!("and attach the contents of this directory.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_plain_tag() {
        let xml = "<foo>sensitive</foo>";
        assert_eq!(
            redact_xml_tag(xml, "foo", "REDACTED"),
            "<foo>REDACTED</foo>"
        );
    }

    #[test]
    fn redact_cdata_tag() {
        let xml = "<hostname><![CDATA[MYNAS]]></hostname>";
        assert_eq!(
            redact_xml_tag(xml, "hostname", "<![CDATA[REDACTED]]>"),
            "<hostname><![CDATA[REDACTED]]></hostname>"
        );
    }

    #[test]
    fn redact_preserves_surrounding_content() {
        let xml = "<a>1</a><serial_number>ABC123</serial_number><b>2</b>";
        let result = redact_xml_tag(xml, "serial_number", "REDACTED");
        assert_eq!(
            result,
            "<a>1</a><serial_number>REDACTED</serial_number><b>2</b>"
        );
    }

    #[test]
    fn redact_multiple_occurrences() {
        let xml = "<ip>1.2.3.4</ip><ip>5.6.7.8</ip>";
        let result = redact_xml_tag(xml, "ip", "x.x.x.x");
        assert_eq!(result, "<ip>x.x.x.x</ip><ip>x.x.x.x</ip>");
    }

    #[test]
    fn anonymize_xml_scrubs_known_fields() {
        let xml = "<serial_number>ABC123</serial_number>\
                   <hostname><![CDATA[MYNAS]]></hostname>\
                   <server_name><![CDATA[MYNAS]]></server_name>\
                   <eth_ip1>192.168.1.50</eth_ip1>\
                   <dns1>192.168.1.1</dns1>\
                   <DNS_LIST><![CDATA[10.10.30.1]]></DNS_LIST>";
        let result = anonymize_xml(xml);
        assert!(!result.contains("ABC123"));
        assert!(!result.contains("MYNAS"));
        assert!(!result.contains("192.168.1.50"));
        assert!(!result.contains("192.168.1.1"));
        assert!(!result.contains("10.10.30.1"));
        assert!(result.contains("REDACTED"));
        assert!(result.contains("x.x.x.x"));
    }

    #[test]
    fn anonymize_json_owners_replaces_personal_names() {
        let json = r#"{"datas":[{"owner":"ruben","group":"everyone"},{"owner":"admin","group":"administrators"}]}"#;
        let result = anonymize_json_owners(json);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let entries = v["datas"].as_array().unwrap();
        assert_eq!(entries[0]["owner"], "user");
        assert_eq!(entries[0]["group"], "everyone");
        assert_eq!(entries[1]["owner"], "admin");
        assert_eq!(entries[1]["group"], "administrators");
    }

    #[test]
    fn anonymize_json_owners_handles_invalid_json() {
        let bad = "not json";
        assert_eq!(anonymize_json_owners(bad), bad);
    }
}
