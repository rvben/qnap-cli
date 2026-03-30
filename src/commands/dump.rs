use anyhow::{Context, Result};
use std::path::Path;

use crate::client::{QnapClient, extract_xml_value};

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
    save!("sysinfo.xml", &sysinfo);

    let volumes = client
        .get_cgi(
            "/cgi-bin/disk/disk_manage.cgi",
            &[("func", "extra_get"), ("extra_vol_index", "1")],
        )
        .await?;
    save!("volumes.xml", &volumes);

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
    save!("files-share.json", &files_share);

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
