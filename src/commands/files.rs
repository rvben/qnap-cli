use anyhow::{Result, bail};
use serde::Deserialize;

use crate::client::QnapClient;
use crate::output::{FileRow, print_files};

const PAGE_SIZE: usize = 200;

#[derive(Debug, Deserialize)]
struct FileListResponse {
    status: Option<u64>,
    datas: Option<Vec<FileEntry>>,
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    filename: Option<String>,
    isfolder: Option<u8>,
    filesize: Option<String>,
    mt: Option<String>,
}

fn check_list_status(status: Option<u64>, path: &str) -> Result<()> {
    match status {
        Some(0) | None => Ok(()),
        Some(5) => bail!("path not found: {}", path),
        Some(20) => bail!("permission denied: {}", path),
        Some(code) => bail!("error listing {}: status={}", path, code),
    }
}

pub async fn list(client: &QnapClient, path: &str, all: bool, json: bool) -> Result<()> {
    let mut rows: Vec<FileRow> = Vec::new();
    let mut start = 0usize;

    loop {
        let limit_s = PAGE_SIZE.to_string();
        let start_s = start.to_string();
        let resp: FileListResponse = client
            .get_json(
                "/cgi-bin/filemanager/utilRequest.cgi",
                &[
                    ("func", "get_list"),
                    ("path", path),
                    ("limit", &limit_s),
                    ("start", &start_s),
                    ("type", "both"),
                    ("sortby", "filename"),
                    ("order", "ASC"),
                ],
            )
            .await?;

        check_list_status(resp.status, path)?;

        let page = resp.datas.unwrap_or_default();
        let page_len = page.len();

        rows.extend(page.into_iter().map(|f| FileRow {
            file_type: if f.isfolder == Some(1) {
                "dir".to_string()
            } else {
                "file".to_string()
            },
            name: f.filename.unwrap_or_else(|| "?".to_string()),
            size: f.filesize.unwrap_or_else(|| "-".to_string()),
            modified: f.mt.unwrap_or_else(|| "-".to_string()),
        }));

        if page_len < PAGE_SIZE || !all {
            if !all && page_len == PAGE_SIZE {
                eprintln!(
                    "  (showing first {} results — use --all to fetch everything)",
                    PAGE_SIZE
                );
            }
            break;
        }

        start += PAGE_SIZE;
    }

    print_files(&rows, json);
    Ok(())
}

pub async fn stat(client: &QnapClient, path: &str, json: bool) -> Result<()> {
    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "stat"), ("path", path)],
        )
        .await?;

    // Check top-level status before extracting data
    let status = resp.get("status").and_then(|s| s.as_u64());
    match status {
        Some(0) | None => {}
        Some(5) => bail!("path not found: {}", path),
        Some(20) => bail!("permission denied: {}", path),
        Some(code) => bail!("error getting stat for {}: status={}", path, code),
    }

    // The API wraps metadata in a `datas` array; extract the first entry
    let entry = resp
        .get("datas")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .unwrap_or(&resp);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(entry).unwrap_or_default()
        );
    } else if let Some(map) = entry.as_object() {
        let pairs: Vec<(String, String)> = map
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => "-".to_string(),
                    other => other.to_string(),
                };
                (k.clone(), val)
            })
            .collect();
        crate::output::print_kv(&pairs);
    } else {
        println!("{}", entry);
    }
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    const FILES_SHARE: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/files-share.json");

    #[test]
    fn fixture_files_share_parses() {
        let resp: FileListResponse =
            serde_json::from_str(FILES_SHARE).expect("failed to parse files-share fixture");
        assert!(
            matches!(resp.status, None | Some(0)),
            "unexpected error status in files-share fixture: {:?}",
            resp.status
        );
    }

    #[test]
    fn fixture_files_share_entries_have_names() {
        let resp: FileListResponse = serde_json::from_str(FILES_SHARE).unwrap();
        let entries = resp.datas.unwrap_or_default();
        assert!(!entries.is_empty(), "no entries in files-share fixture");
        for e in &entries {
            assert!(
                e.filename
                    .as_deref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false),
                "file entry has empty or missing filename"
            );
        }
    }

    #[test]
    fn fixture_files_share_check_list_status_ok() {
        let resp: FileListResponse = serde_json::from_str(FILES_SHARE).unwrap();
        assert!(
            check_list_status(resp.status, "/Backups").is_ok(),
            "check_list_status failed for files-share fixture"
        );
    }
}
