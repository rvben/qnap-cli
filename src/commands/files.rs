use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::client::QnapClient;
use crate::output::print_value;

#[derive(Debug, Deserialize)]
struct FileListResponse {
    status: Option<u64>,
    data: Option<FileListData>,
}

#[derive(Debug, Deserialize)]
struct FileListData {
    files: Option<Vec<FileEntry>>,
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    filename: Option<String>,
    is_folder: Option<u8>,
    filesize: Option<String>,
    mt: Option<String>,
}

pub async fn list(client: &QnapClient, path: &str, json: bool) -> Result<()> {
    let resp: FileListResponse = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[
                ("func", "get_list"),
                ("path", path),
                ("limit", "200"),
                ("start", "0"),
                ("type", "both"),
                ("sortby", "filename"),
                ("order", "ASC"),
            ],
        )
        .await?;

    if resp.status != Some(1) {
        bail!("file list request returned non-success status");
    }

    let files = resp.data.and_then(|d| d.files).unwrap_or_default();

    if json {
        let entries: Vec<Value> = files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.filename,
                    "type": if f.is_folder == Some(1) { "dir" } else { "file" },
                    "size": f.filesize,
                    "modified": f.mt,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for f in &files {
            let name = f.filename.as_deref().unwrap_or("?");
            let kind = if f.is_folder == Some(1) { "d" } else { "-" };
            let size = f.filesize.as_deref().unwrap_or("-");
            let mtime = f.mt.as_deref().unwrap_or("-");
            println!("{} {:>12}  {}  {}", kind, size, mtime, name);
        }
        println!("\n  {} item(s)", files.len());
    }

    Ok(())
}

pub async fn stat(client: &QnapClient, path: &str, json: bool) -> Result<()> {
    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "stat"), ("path", path)],
        )
        .await?;
    print_value(&resp, json);
    Ok(())
}
