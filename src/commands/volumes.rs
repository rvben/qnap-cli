use anyhow::Result;
use roxmltree::Document;
use serde::Serialize;

use crate::client::{QnapClient, parse_xml, xml_value, xml_value_in};
use crate::output::{DiskRow, VolumeRow, fmt_temp, print_volumes};

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

#[derive(Debug, Serialize, PartialEq)]
struct VolumeOutput {
    label: String,
    status: String,
    status_code: Option<i64>,
    pool_id: Option<u64>,
    volume_type: String,
}

#[derive(Debug, Serialize, PartialEq)]
struct DiskOutput {
    slot: u64,
    model: String,
    kind: String,
    temp_c: Option<f64>,
}

#[derive(Debug, Serialize, PartialEq)]
struct VolumesOutput {
    volumes: Vec<VolumeOutput>,
    disks: Vec<DiskOutput>,
}

fn build_report(vol_doc: &Document<'_>, sys_doc: &Document<'_>) -> VolumesOutput {
    let mut volumes = Vec::new();
    for row in vol_doc
        .descendants()
        .filter(|node| node.has_tag_name("row"))
    {
        let status_code_raw = xml_value_in(row, "vol_status").unwrap_or_default();
        let status_code = status_code_raw.trim().parse::<i64>().ok();
        let volume_type = xml_value_in(row, "lv_type")
            .unwrap_or_default()
            .trim()
            .trim_start_matches("qnap_")
            .to_string();

        volumes.push(VolumeOutput {
            label: xml_value_in(row, "vol_label").unwrap_or_default(),
            status: vol_status_label(&status_code_raw).to_string(),
            status_code,
            pool_id: xml_value_in(row, "poolID").and_then(|value| value.parse().ok()),
            volume_type,
        });
    }

    let disk_count: usize = xml_value(sys_doc, "disk_num")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    let mut disks = Vec::new();
    for index in 1..=disk_count {
        let installed = xml_value(sys_doc, &format!("disk_installed{}", index)).unwrap_or_default();
        if installed != "1" {
            continue;
        }

        let temp_c = xml_value(sys_doc, &format!("tempc{}", index))
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| *value > 0.0);
        let is_ssd = xml_value(sys_doc, &format!("hd_is_ssd{}", index)).unwrap_or_default();

        disks.push(DiskOutput {
            slot: index as u64,
            model: xml_value(sys_doc, &format!("hd_pd_alias{}", index)).unwrap_or_default(),
            kind: if is_ssd == "1" {
                "SSD".to_string()
            } else {
                "HDD".to_string()
            },
            temp_c,
        });
    }

    VolumesOutput { volumes, disks }
}

fn human_rows(report: &VolumesOutput) -> (Vec<VolumeRow>, Vec<DiskRow>) {
    let volumes = report
        .volumes
        .iter()
        .map(|volume| VolumeRow {
            label: volume.label.clone(),
            status: volume.status.clone(),
            pool: volume
                .pool_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            vol_type: volume.volume_type.clone(),
        })
        .collect();

    let disks = report
        .disks
        .iter()
        .map(|disk| DiskRow {
            slot: disk.slot.to_string(),
            model: disk.model.clone(),
            kind: disk.kind.clone(),
            temp: disk
                .temp_c
                .map(|value| fmt_temp(&value.to_string()))
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    (volumes, disks)
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

    let vol_doc = parse_xml(&vol_body)?;
    let sys_doc = parse_xml(&sys_body)?;
    let report = build_report(&vol_doc, &sys_doc);

    if report.volumes.is_empty() && !json {
        let has_rows = vol_doc.descendants().any(|node| node.has_tag_name("row"));
        if has_rows {
            eprintln!("Warning: found <row> elements but could not parse volume fields.");
        } else {
            eprintln!("Warning: no volumes found in API response.");
        }
        eprintln!("  Your firmware may use a different format than tested versions (QTS 4.3+).");
        eprintln!("  Run `qnap dump ./debug/` and open a GitHub issue with the output.");
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
        return Ok(());
    }

    let (volumes, disks) = human_rows(&report);
    print_volumes(&volumes, &disks);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSINFO: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/sysinfo.xml");
    const VOLUMES: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/volumes.xml");

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

    #[test]
    fn test_build_report_parses_structured_values() {
        let vol_doc = parse_xml(VOLUMES).unwrap();
        let sys_doc = parse_xml(SYSINFO).unwrap();
        let report = build_report(&vol_doc, &sys_doc);

        assert_eq!(
            report.volumes,
            vec![VolumeOutput {
                label: "DataVol1".to_string(),
                status: "ready".to_string(),
                status_code: Some(-7),
                pool_id: Some(1),
                volume_type: "thick".to_string(),
            }]
        );
        assert_eq!(report.disks.len(), 3);
        assert_eq!(report.disks[0].temp_c, None);
        assert_eq!(report.disks[1].temp_c, Some(38.0));
    }
}
