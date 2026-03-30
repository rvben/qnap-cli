use anyhow::Result;
use serde::Deserialize;

use crate::client::QnapClient;
use crate::output::{ShareRow, print_shares};

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

    let rows: Vec<ShareRow> = entries
        .into_iter()
        .map(|e| ShareRow {
            name: e.text,
            path: e.id,
            items: e
                .real_total
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    print_shares(&rows, json);
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    const SHARES: &str = include_str!("../../tests/fixtures/ts-xa28a-qts52/shares.json");

    #[test]
    fn fixture_shares_parses() {
        let entries: Vec<ShareEntry> =
            serde_json::from_str(SHARES).expect("failed to parse shares fixture");
        assert!(!entries.is_empty(), "no shares found in fixture");
    }

    #[test]
    fn fixture_shares_have_id_and_name() {
        let entries: Vec<ShareEntry> = serde_json::from_str(SHARES).unwrap();
        for e in &entries {
            assert!(!e.id.is_empty(), "share entry has empty id");
            assert!(!e.text.is_empty(), "share entry has empty text");
        }
    }
}
