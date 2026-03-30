use owo_colors::OwoColorize;
use std::io::IsTerminal;
use tabled::{Table, Tabled};

fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
}

/// Print aligned key-value pairs.
pub fn print_kv(pairs: &[(String, String)]) {
    let max_key = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in pairs {
        println!("  {:<width$}  {}", k, v, width = max_key);
    }
}

/// Format a temperature value with color coding for terminal output.
///
/// Green below 45°C, yellow 45–59°C, red 60°C and above.
pub fn fmt_temp(temp_str: &str) -> String {
    let Ok(t) = temp_str.trim().parse::<f64>() else {
        return temp_str.to_string();
    };
    let display = format!("{}°C", t as i64);
    if !use_color() {
        return display;
    }
    if t >= 60.0 {
        display.red().to_string()
    } else if t >= 45.0 {
        display.yellow().to_string()
    } else {
        display.green().to_string()
    }
}

/// Format a volume status string with color coding.
pub fn fmt_vol_status(status: &str) -> String {
    if !use_color() {
        return status.to_string();
    }
    match status {
        "ready" => status.green().to_string(),
        "degraded" | "error" | "not active" => status.red().to_string(),
        "not ready" => status.yellow().to_string(),
        _ => status.to_string(),
    }
}

// ── Volume output ────────────────────────────────────────────────────────────

#[derive(Tabled)]
pub struct VolumeRow {
    #[tabled(rename = "Label")]
    pub label: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Pool")]
    pub pool: String,
    #[tabled(rename = "Type")]
    pub vol_type: String,
}

#[derive(Tabled)]
pub struct DiskRow {
    #[tabled(rename = "Slot")]
    pub slot: String,
    #[tabled(rename = "Model")]
    pub model: String,
    #[tabled(rename = "Kind")]
    pub kind: String,
    #[tabled(rename = "Temp")]
    pub temp: String,
}

pub fn print_volumes(volumes: &[VolumeRow], disks: &[DiskRow], json: bool) {
    if json {
        let vols: Vec<serde_json::Value> = volumes
            .iter()
            .map(|v| {
                serde_json::json!({
                    "label": v.label,
                    "status": v.status,
                    "pool": v.pool,
                    "type": v.vol_type,
                })
            })
            .collect();
        let dsk: Vec<serde_json::Value> = disks
            .iter()
            .map(|d| {
                serde_json::json!({
                    "slot": d.slot,
                    "model": d.model,
                    "kind": d.kind,
                    "temp": d.temp,
                })
            })
            .collect();
        let out = serde_json::json!({ "volumes": vols, "disks": dsk });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return;
    }

    println!("VOLUMES");
    if volumes.is_empty() {
        println!("  (none)");
    } else {
        println!("{}", Table::new(volumes));
    }

    println!();
    println!("DISKS");
    if disks.is_empty() {
        println!("  (none)");
    } else {
        println!("{}", Table::new(disks));
    }
}

// ── Share output ─────────────────────────────────────────────────────────────

#[derive(Tabled)]
pub struct ShareRow {
    #[tabled(rename = "Name")]
    pub name: String,
    #[tabled(rename = "Path")]
    pub path: String,
    #[tabled(rename = "Items")]
    pub items: String,
}

pub fn print_shares(rows: &[ShareRow], json: bool) {
    if json {
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "path": r.path,
                    "items": r.items,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return;
    }
    println!("{}", Table::new(rows));
}

// ── File listing output ───────────────────────────────────────────────────────

#[derive(Tabled)]
pub struct FileRow {
    #[tabled(rename = "Type")]
    pub file_type: String,
    #[tabled(rename = "Name")]
    pub name: String,
    #[tabled(rename = "Size")]
    pub size: String,
    #[tabled(rename = "Modified")]
    pub modified: String,
}

pub fn print_files(rows: &[FileRow], json: bool) {
    if json {
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "type": r.file_type,
                    "name": r.name,
                    "size": r.size,
                    "modified": r.modified,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return;
    }
    println!("{}", Table::new(rows));
    println!("  {} item(s)", rows.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests run without a TTY so use_color() returns false — no ANSI codes in output.

    #[test]
    fn test_fmt_temp_below_threshold() {
        assert_eq!(fmt_temp("32"), "32°C");
        assert_eq!(fmt_temp("0"), "0°C");
        assert_eq!(fmt_temp("44"), "44°C");
    }

    #[test]
    fn test_fmt_temp_at_yellow_threshold() {
        assert_eq!(fmt_temp("45"), "45°C");
        assert_eq!(fmt_temp("59"), "59°C");
    }

    #[test]
    fn test_fmt_temp_at_red_threshold() {
        assert_eq!(fmt_temp("60"), "60°C");
        assert_eq!(fmt_temp("75"), "75°C");
    }

    #[test]
    fn test_fmt_temp_invalid_input() {
        assert_eq!(fmt_temp("N/A"), "N/A");
        assert_eq!(fmt_temp(""), "");
    }

    #[test]
    fn test_fmt_temp_decimal() {
        assert_eq!(fmt_temp("38.5"), "38°C");
    }

    #[test]
    fn test_fmt_vol_status_ready() {
        assert_eq!(fmt_vol_status("ready"), "ready");
    }

    #[test]
    fn test_fmt_vol_status_error_states() {
        assert_eq!(fmt_vol_status("error"), "error");
        assert_eq!(fmt_vol_status("degraded"), "degraded");
        assert_eq!(fmt_vol_status("not active"), "not active");
    }

    #[test]
    fn test_fmt_vol_status_not_ready() {
        assert_eq!(fmt_vol_status("not ready"), "not ready");
    }

    #[test]
    fn test_fmt_vol_status_unknown() {
        assert_eq!(fmt_vol_status("something-else"), "something-else");
    }
}
