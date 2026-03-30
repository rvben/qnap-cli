use anyhow::Result;

use crate::client::QnapClient;
use crate::output::print_value;

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let resp: serde_json::Value = client
        .get_json(
            "/api/v1/share-folders",
            &[("limit", "100"), ("offset", "0")],
        )
        .await?;

    if let Some(data) = resp.get("data") {
        print_value(data, json);
    } else {
        print_value(&resp, json);
    }

    Ok(())
}
