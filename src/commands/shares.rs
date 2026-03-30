use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::QnapClient;
use crate::output::{ShareRow, print_shares};

#[derive(Debug, Deserialize)]
struct ShareEntry {
    id: String,
    text: String,
    real_total: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
struct ShareOutput {
    name: String,
    path: String,
    items_count: Option<u64>,
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let entries: Vec<ShareEntry> = client
        .get_json(
            "/cgi-bin/filemanager/utilRequest.cgi",
            &[("func", "get_tree"), ("node", "share_root")],
        )
        .await?;

    let rows: Vec<ShareOutput> = entries
        .into_iter()
        .map(|entry| ShareOutput {
            name: entry.text,
            path: entry.id,
            items_count: entry.real_total,
        })
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).unwrap_or_default()
        );
        return Ok(());
    }

    let table_rows: Vec<ShareRow> = rows
        .iter()
        .map(|row| ShareRow {
            name: row.name.clone(),
            path: row.path.clone(),
            items: row
                .items_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();
    print_shares(&table_rows);
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
        for entry in &entries {
            assert!(!entry.id.is_empty(), "share entry has empty id");
            assert!(!entry.text.is_empty(), "share entry has empty text");
        }
    }

    #[test]
    fn fixture_shares_output_is_typed() {
        let entries: Vec<ShareEntry> = serde_json::from_str(SHARES).unwrap();
        let rows: Vec<ShareOutput> = entries
            .into_iter()
            .map(|entry| ShareOutput {
                name: entry.text,
                path: entry.id,
                items_count: entry.real_total,
            })
            .collect();
        assert_eq!(rows[0].items_count, Some(2));
    }
}
