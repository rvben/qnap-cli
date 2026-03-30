use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::client::QnapClient;
use crate::output::{FileRow, print_files, print_kv};

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
    epochmt: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
struct FileListItem {
    entry_type: String,
    name: String,
    size_bytes: Option<u64>,
    modified: Option<String>,
    modified_epoch: Option<u64>,
}

#[derive(Debug, Serialize)]
struct FileStatOutput {
    path: String,
    name: Option<String>,
    entry_type: Option<String>,
    size_bytes: Option<u64>,
    modified: Option<String>,
    modified_epoch: Option<u64>,
    owner: Option<String>,
    group: Option<String>,
    permissions: Option<String>,
    exists: Option<bool>,
    is_link: Option<bool>,
    raw: serde_json::Value,
}

fn check_list_status(status: Option<u64>, path: &str) -> Result<()> {
    match status {
        Some(0) | None => Ok(()),
        Some(5) => bail!("path not found: {}", path),
        Some(20) => bail!("permission denied: {}", path),
        Some(code) => bail!("error listing {}: status={}", path, code),
    }
}

/// Split a remote path into (parent_dir, filename).
///
/// "/Backups/files/foo.txt" → ("/Backups/files", "foo.txt")
/// "/Backups"               → ("/",              "Backups")
fn split_path(path: &str) -> (&str, &str) {
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return ("/", "");
    }
    match path.rfind('/') {
        None => ("/", path),
        Some(0) => ("/", &path[1..]),
        Some(i) => (&path[..i], &path[i + 1..]),
    }
}

/// Check the status field of a mutating file operation response.
///
/// QNAP uses 0 or 1 to indicate success depending on firmware version.
fn check_op_status(resp: &serde_json::Value, op: &str, path: &str) -> Result<()> {
    match resp.get("status").and_then(|s| s.as_u64()) {
        Some(0) | Some(1) | None => Ok(()),
        Some(2) => bail!("{}: destination already exists: {}", op, path),
        Some(5) => bail!("{}: path not found: {}", op, path),
        Some(20) => bail!("{}: permission denied: {}", op, path),
        Some(code) => bail!("{} failed for {}: status={}", op, path, code),
    }
}

fn file_type_label(is_folder: Option<u8>) -> String {
    if is_folder == Some(1) {
        "dir".to_string()
    } else {
        "file".to_string()
    }
}

fn parse_size_bytes(value: Option<String>) -> Option<u64> {
    value.and_then(|raw| raw.trim().parse::<u64>().ok())
}

fn list_item_from_entry(entry: FileEntry) -> FileListItem {
    FileListItem {
        entry_type: file_type_label(entry.isfolder),
        name: entry.filename.unwrap_or_else(|| "?".to_string()),
        size_bytes: parse_size_bytes(entry.filesize),
        modified: entry.mt,
        modified_epoch: entry.epochmt,
    }
}

fn human_row(item: &FileListItem) -> FileRow {
    FileRow {
        file_type: item.entry_type.clone(),
        name: item.name.clone(),
        size: item
            .size_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        modified: item.modified.clone().unwrap_or_else(|| "-".to_string()),
    }
}

fn bool_from_u8(value: Option<u8>) -> Option<bool> {
    value.map(|value| value != 0)
}

fn stat_output(path: &str, entry: &serde_json::Value) -> FileStatOutput {
    let is_folder = entry
        .get("isfolder")
        .and_then(|value| value.as_u64())
        .map(|value| value as u8);
    let size_bytes = entry
        .get("filesize")
        .and_then(|value| value.as_str())
        .and_then(|value| value.trim().parse::<u64>().ok());

    FileStatOutput {
        path: path.to_string(),
        name: entry
            .get("filename")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        entry_type: Some(file_type_label(is_folder)),
        size_bytes,
        modified: entry
            .get("mt")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        modified_epoch: entry.get("epochmt").and_then(|value| value.as_u64()),
        owner: entry
            .get("owner")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        group: entry
            .get("group")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        permissions: entry
            .get("privilege")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        exists: bool_from_u8(
            entry
                .get("exist")
                .and_then(|value| value.as_u64())
                .map(|value| value as u8),
        ),
        is_link: bool_from_u8(
            entry
                .get("is_link")
                .and_then(|value| value.as_u64())
                .map(|value| value as u8),
        ),
        raw: entry.clone(),
    }
}

pub async fn list(client: &QnapClient, path: &str, all: bool, json: bool) -> Result<()> {
    let mut items: Vec<FileListItem> = Vec::new();
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
        items.extend(page.into_iter().map(list_item_from_entry));

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

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).unwrap_or_default()
        );
        return Ok(());
    }

    let rows: Vec<FileRow> = items.iter().map(human_row).collect();
    print_files(&rows);
    Ok(())
}

pub async fn stat(client: &QnapClient, path: &str, json: bool) -> Result<()> {
    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "stat"), ("path", path)],
        )
        .await?;

    let status = resp.get("status").and_then(|value| value.as_u64());
    match status {
        Some(0) | None => {}
        Some(5) => bail!("path not found: {}", path),
        Some(20) => bail!("permission denied: {}", path),
        Some(code) => bail!("error getting stat for {}: status={}", path, code),
    }

    let entry = resp
        .get("datas")
        .and_then(|value| value.as_array())
        .and_then(|values| values.first())
        .unwrap_or(&resp);
    let stat = stat_output(path, entry);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&stat).unwrap_or_default()
        );
        return Ok(());
    }

    let mut pairs = Vec::new();
    pairs.push(("path".to_string(), stat.path));
    if let Some(value) = stat.name {
        pairs.push(("name".to_string(), value));
    }
    if let Some(value) = stat.entry_type {
        pairs.push(("type".to_string(), value));
    }
    if let Some(value) = stat.size_bytes {
        pairs.push(("size_bytes".to_string(), value.to_string()));
    }
    if let Some(value) = stat.modified {
        pairs.push(("modified".to_string(), value));
    }
    if let Some(value) = stat.modified_epoch {
        pairs.push(("modified_epoch".to_string(), value.to_string()));
    }
    if let Some(value) = stat.owner {
        pairs.push(("owner".to_string(), value));
    }
    if let Some(value) = stat.group {
        pairs.push(("group".to_string(), value));
    }
    if let Some(value) = stat.permissions {
        pairs.push(("permissions".to_string(), value));
    }
    if let Some(value) = stat.exists {
        pairs.push(("exists".to_string(), value.to_string()));
    }
    if let Some(value) = stat.is_link {
        pairs.push(("is_link".to_string(), value.to_string()));
    }

    print_kv(&pairs);
    Ok(())
}

pub async fn mkdir(client: &QnapClient, path: &str) -> Result<()> {
    let (parent, name) = split_path(path);
    if name.is_empty() {
        bail!("invalid path: {}", path);
    }
    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[
                ("func", "createdir"),
                ("dest_path", parent),
                ("dest_folder", name),
            ],
        )
        .await?;
    check_op_status(&resp, "mkdir", path)
}

pub async fn rm(client: &QnapClient, path: &str) -> Result<()> {
    let (parent, name) = split_path(path);
    if name.is_empty() {
        bail!("cannot delete root path");
    }
    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[
                ("func", "delete"),
                ("file_total", "1"),
                ("file_name[0]", name),
                ("file_path[0]", parent),
            ],
        )
        .await?;
    check_op_status(&resp, "rm", path)
}

pub async fn mv(client: &QnapClient, src: &str, dst: &str) -> Result<()> {
    let (src_parent, src_name) = split_path(src);
    let (dst_parent, dst_name) = split_path(dst);

    if src_name.is_empty() {
        bail!("invalid source path: {}", src);
    }
    if dst_name.is_empty() {
        bail!("invalid destination path: {}", dst);
    }

    if src_parent == dst_parent {
        // Rename within the same directory
        let resp: serde_json::Value = client
            .get_json(
                "/cgi-bin/filemanager/utilRequest.cgi",
                &[
                    ("func", "rename"),
                    ("path", src_parent),
                    ("source_name", src_name),
                    ("dest_name", dst_name),
                ],
            )
            .await?;
        check_op_status(&resp, "mv", src)
    } else if src_name == dst_name {
        // Move to a different directory, keeping the same filename
        let resp: serde_json::Value = client
            .get_json(
                "/cgi-bin/filemanager/utilRequest.cgi",
                &[
                    ("func", "move"),
                    ("source_path", src_parent),
                    ("source_file", src_name),
                    ("dest_path", dst_parent),
                    ("source_total", "1"),
                    ("overwrite", "0"),
                ],
            )
            .await?;
        check_op_status(&resp, "mv", src)
    } else {
        // Move to a different directory then rename
        let resp: serde_json::Value = client
            .get_json(
                "/cgi-bin/filemanager/utilRequest.cgi",
                &[
                    ("func", "move"),
                    ("source_path", src_parent),
                    ("source_file", src_name),
                    ("dest_path", dst_parent),
                    ("source_total", "1"),
                    ("overwrite", "0"),
                ],
            )
            .await?;
        check_op_status(&resp, "mv", src)?;

        let resp: serde_json::Value = client
            .get_json(
                "/cgi-bin/filemanager/utilRequest.cgi",
                &[
                    ("func", "rename"),
                    ("path", dst_parent),
                    ("source_name", src_name),
                    ("dest_name", dst_name),
                ],
            )
            .await?;
        check_op_status(&resp, "mv (rename)", dst)
    }
}

pub async fn cp(client: &QnapClient, src: &str, dst: &str, overwrite: bool) -> Result<()> {
    let (src_parent, src_name) = split_path(src);
    let (dst_parent, _) = split_path(dst);

    if src_name.is_empty() {
        bail!("invalid source path: {}", src);
    }

    let resp: serde_json::Value = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[
                ("func", "copy"),
                ("source_path", src_parent),
                ("source_file", src_name),
                ("dest_path", dst_parent),
                ("source_total", "1"),
                ("overwrite", if overwrite { "1" } else { "0" }),
            ],
        )
        .await?;
    check_op_status(&resp, "cp", src)
}

pub async fn upload(
    client: &QnapClient,
    local: &Path,
    remote_dir: &str,
    overwrite: bool,
) -> Result<()> {
    let filename = local
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("invalid local path: {}", local.display()))?;

    let data = std::fs::read(local)
        .with_context(|| format!("failed to read {}", local.display()))?;

    let resp = client.upload_file(remote_dir, filename, data, overwrite).await?;
    check_op_status(&resp, "upload", filename)
}

pub async fn download(client: &QnapClient, remote: &str, local: Option<&Path>) -> Result<()> {
    let (source_path, source_file) = split_path(remote);
    if source_file.is_empty() {
        bail!("cannot download a directory");
    }

    let resp = client.get_file_response(source_path, source_file).await?;

    // Determine output destination
    let out_path = local.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .join(source_file)
    });

    let mut file = std::fs::File::create(&out_path)
        .with_context(|| format!("failed to create {}", out_path.display()))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("error reading download stream")?;
        std::io::Write::write_all(&mut file, &chunk)
            .with_context(|| format!("failed to write to {}", out_path.display()))?;
    }

    eprintln!("  saved to {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_deep() {
        assert_eq!(split_path("/Backups/files/foo.txt"), ("/Backups/files", "foo.txt"));
    }

    #[test]
    fn split_path_share_root() {
        assert_eq!(split_path("/Backups"), ("/", "Backups"));
    }

    #[test]
    fn split_path_trailing_slash() {
        assert_eq!(split_path("/Backups/files/"), ("/Backups", "files"));
    }

    #[test]
    fn split_path_no_leading_slash() {
        assert_eq!(split_path("file.txt"), ("/", "file.txt"));
    }

    #[test]
    fn check_op_status_success_zero() {
        let v = serde_json::json!({"status": 0});
        assert!(check_op_status(&v, "op", "/path").is_ok());
    }

    #[test]
    fn check_op_status_success_one() {
        let v = serde_json::json!({"status": 1});
        assert!(check_op_status(&v, "op", "/path").is_ok());
    }

    #[test]
    fn check_op_status_not_found() {
        let v = serde_json::json!({"status": 5});
        let err = check_op_status(&v, "rm", "/missing").unwrap_err();
        assert!(err.to_string().contains("path not found"));
    }

    #[test]
    fn check_op_status_permission_denied() {
        let v = serde_json::json!({"status": 20});
        let err = check_op_status(&v, "mkdir", "/locked").unwrap_err();
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn check_op_status_already_exists() {
        let v = serde_json::json!({"status": 2});
        let err = check_op_status(&v, "cp", "/dst").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
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
        for entry in &entries {
            assert!(
                entry
                    .filename
                    .as_deref()
                    .map(|value| !value.is_empty())
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

    #[test]
    fn fixture_files_share_items_are_typed() {
        let resp: FileListResponse = serde_json::from_str(FILES_SHARE).unwrap();
        let items: Vec<FileListItem> = resp
            .datas
            .unwrap_or_default()
            .into_iter()
            .map(list_item_from_entry)
            .collect();
        assert_eq!(items[0].entry_type, "dir");
        assert_eq!(items[0].size_bytes, Some(4096));
        assert!(items[0].modified_epoch.is_some());
    }

    #[test]
    fn stat_output_preserves_typed_and_raw_fields() {
        let resp: serde_json::Value = serde_json::from_str(FILES_SHARE).unwrap();
        let entry = resp
            .get("datas")
            .and_then(|value| value.as_array())
            .and_then(|values| values.first())
            .unwrap();

        let stat = stat_output("/Backups/@Recycle", entry);
        assert_eq!(stat.entry_type.as_deref(), Some("dir"));
        assert_eq!(stat.size_bytes, Some(4096));
        assert_eq!(stat.modified_epoch, Some(1771262677));
        assert!(stat.raw.get("filename").is_some());
    }
}
