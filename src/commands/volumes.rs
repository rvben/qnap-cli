use anyhow::Result;

use crate::client::{QnapClient, extract_xml_value};
use crate::output::{DiskRow, VolumeRow, fmt_temp, fmt_vol_status, print_volumes};

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
    let vol_body = client
        .get_cgi(
            "/cgi-bin/disk/disk_manage.cgi",
            &[("func", "extra_get"), ("extra_vol_index", "1")],
        )
        .await?;

    let sys_body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    // Parse volumes from repeated <row> blocks
    let mut volumes: Vec<VolumeRow> = Vec::new();
    let mut remaining = vol_body.as_str();
    while let Some(start) = remaining.find("<row>") {
        remaining = &remaining[start + "<row>".len()..];
        let end = remaining.find("</row>").unwrap_or(remaining.len());
        let block = &remaining[..end];
        remaining = &remaining[end..];

        let status_code = extract_xml_value(block, "vol_status").unwrap_or_default();
        let label = extract_xml_value(block, "vol_label").unwrap_or_default();
        let pool = extract_xml_value(block, "poolID").unwrap_or_default();
        let vol_type = extract_xml_value(block, "lv_type").unwrap_or_default();
        let status_text = vol_status_label(&status_code);

        volumes.push(VolumeRow {
            label,
            status: fmt_vol_status(status_text),
            pool,
            vol_type,
        });
    }

    // Parse installed disks from sysinfo
    let disk_count: usize = extract_xml_value(&sys_body, "disk_num")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let mut disks: Vec<DiskRow> = Vec::new();
    for i in 1..=disk_count {
        let installed =
            extract_xml_value(&sys_body, &format!("disk_installed{}", i)).unwrap_or_default();
        if installed != "1" {
            continue;
        }
        let model = extract_xml_value(&sys_body, &format!("hd_pd_alias{}", i)).unwrap_or_default();
        let temp_raw = extract_xml_value(&sys_body, &format!("tempc{}", i)).unwrap_or_default();
        let is_ssd = extract_xml_value(&sys_body, &format!("hd_is_ssd{}", i)).unwrap_or_default();

        let temp = if temp_raw.is_empty() || temp_raw == "0" {
            "-".to_string()
        } else if json {
            format!("{}°C", temp_raw.trim())
        } else {
            fmt_temp(&temp_raw)
        };

        disks.push(DiskRow {
            slot: i.to_string(),
            model,
            kind: if is_ssd == "1" { "SSD" } else { "HDD" }.to_string(),
            temp,
        });
    }

    if volumes.is_empty() && !json {
        let has_rows = vol_body.contains("<row>");
        if has_rows {
            eprintln!("Warning: found <row> elements but could not parse volume fields.");
        } else {
            eprintln!("Warning: no volumes found in API response.");
        }
        eprintln!("  Your firmware may use a different format than tested versions (QTS 4.3+).");
        eprintln!("  Run `qnap dump ./debug/` and open a GitHub issue with the output.");
    }

    print_volumes(&volumes, &disks, json);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vol_status_label_ready_codes() {
        assert_eq!(vol_status_label("0"), "ready");
        assert_eq!(vol_status_label("-7"), "ready");
    }

    #[test]
    fn test_vol_status_label_error_codes() {
        assert_eq!(vol_status_label("-1"), "not ready");
        assert_eq!(vol_status_label("-4"), "error");
        assert_eq!(vol_status_label("-5"), "degraded");
        assert_eq!(vol_status_label("-6"), "not active");
    }

    #[test]
    fn test_vol_status_label_unknown() {
        assert_eq!(vol_status_label("99"), "unknown");
        assert_eq!(vol_status_label(""), "unknown");
        assert_eq!(vol_status_label("abc"), "unknown");
    }

    #[test]
    fn test_vol_status_label_trims_whitespace() {
        assert_eq!(vol_status_label("  0  "), "ready");
        assert_eq!(vol_status_label(" -5 "), "degraded");
    }
}
