use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::client::QnapClient;
use crate::output::print_value;

#[derive(Debug, Deserialize)]
struct ShareEntry {
    id: String,
    text: String,
    real_total: Option<u64>,
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let entries: Vec<ShareEntry> = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "get_tree"), ("node", "share_root")],
        )
        .await?;

    let items: Vec<Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.text,
                "path": e.id,
                "items": e.real_total,
            })
        })
        .collect();

    print_value(&Value::Array(items), json);
    Ok(())
}
