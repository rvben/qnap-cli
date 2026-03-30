use anyhow::Result;
use serde::Serialize;

use crate::client::{QnapClient, Uptime, parse_uptime, parse_xml, xml_value};
use crate::output::{fmt_temp, print_kv};

#[derive(Debug, Serialize, PartialEq)]
struct UptimeOutput {
    display: String,
    days: u64,
    hours: u64,
    minutes: u64,
    seconds: u64,
    total_seconds: u64,
}

#[derive(Debug, Serialize, PartialEq)]
struct StatusOutput {
    cpu_usage_pct: Option<f64>,
    mem_total_mb: Option<u64>,
    mem_used_mb: Option<u64>,
    mem_free_mb: Option<u64>,
    temp_c: Option<f64>,
    uptime: Option<UptimeOutput>,
}

impl From<Uptime> for UptimeOutput {
    fn from(uptime: Uptime) -> Self {
        Self {
            display: uptime.display(),
            days: uptime.days,
            hours: uptime.hours,
            minutes: uptime.minutes,
            seconds: uptime.seconds,
            total_seconds: uptime.total_seconds(),
        }
    }
}

fn parse_metric_f64(value: Option<String>) -> Option<f64> {
    let value = value?;
    let cleaned = value.trim().trim_end_matches('%').trim();
    cleaned.parse::<f64>().ok()
}

fn parse_metric_mb(value: Option<String>) -> Option<u64> {
    parse_metric_f64(value).map(|number| number.round() as u64)
}

fn build_status(body: &str) -> Result<StatusOutput> {
    let doc = parse_xml(body)?;
    let cpu_usage_pct = parse_metric_f64(xml_value(&doc, "cpu_usage").map(|value| {
        value
            .replace('%', "")
            .split_whitespace()
            .collect::<String>()
    }));

    let mem_total_mb = parse_metric_mb(xml_value(&doc, "total_memory"));
    let mem_free_mb = parse_metric_mb(xml_value(&doc, "free_memory"));
    let mem_used_mb = match (mem_total_mb, mem_free_mb) {
        (Some(total), Some(free)) if total >= free => Some(total - free),
        _ => None,
    };

    let temp_c = parse_metric_f64(xml_value(&doc, "sys_tempc"));

    Ok(StatusOutput {
        cpu_usage_pct,
        mem_total_mb,
        mem_used_mb,
        mem_free_mb,
        temp_c,
        uptime: parse_uptime(&doc).map(Into::into),
    })
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    let status = build_status(&body)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).unwrap_or_default()
        );
        return Ok(());
    }

    let mut pairs: Vec<(String, String)> = Vec::new();

    if let Some(cpu) = status.cpu_usage_pct {
        pairs.push(("cpu_usage".into(), format!("{:.1}%", cpu)));
    }
    if let Some(total) = status.mem_total_mb {
        pairs.push(("mem_total_mb".into(), total.to_string()));
    }
    if let Some(used) = status.mem_used_mb {
        pairs.push(("mem_used_mb".into(), used.to_string()));
    }
    if let Some(free) = status.mem_free_mb {
        pairs.push(("mem_free_mb".into(), free.to_string()));
    }
    if let Some(temp_c) = status.temp_c {
        pairs.push(("temp".into(), fmt_temp(&temp_c.to_string())));
    }
    if let Some(uptime) = status.uptime {
        pairs.push(("uptime".into(), uptime.display));
    }

    print_kv(&pairs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSINFO: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/sysinfo.xml");

    #[test]
    fn build_status_parses_fixture() {
        let status = build_status(SYSINFO).unwrap();
        assert!(status.cpu_usage_pct.is_some(), "cpu_usage_pct missing");
        assert!(status.mem_total_mb.unwrap_or(0) > 0, "mem_total_mb missing");
        assert!(status.temp_c.is_some(), "temp_c missing");
        let uptime = status.uptime.expect("uptime missing");
        assert!(uptime.hours < 24, "hours out of range: {}", uptime.hours);
        assert!(uptime.minutes < 60, "minutes out of range: {}", uptime.minutes);
        assert!(!uptime.display.is_empty());
    }
}
