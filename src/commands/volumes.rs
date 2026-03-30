use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::client::QnapClient;
use crate::output::print_value;

// Returned by the file manager's share tree — each share includes its volume path
#[derive(Debug, Deserialize)]
struct ShareEntry {
    id: String,
    text: String,
    real_total: Option<u64>,
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    // Get share list (available without admin) and disk usage per share
    let shares: Vec<ShareEntry> = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "get_tree"), ("node", "share_root")],
        )
        .await?;

    // For each share, fetch its disk usage via get_list (total/used not available
    // without admin storage API access, so show what we can)
    let mut results: Vec<Value> = Vec::new();
    for share in &shares {
        let entry = serde_json::json!({
            "share": share.text,
            "path": share.id,
            "items": share.real_total.unwrap_or(0),
        });
        results.push(entry);
    }

    if results.is_empty() {
        eprintln!("no shares accessible (storage detail requires admin)");
    }

    print_value(&Value::Array(results), json);
    Ok(())
}
