use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::output::print_kv;

#[derive(Serialize)]
struct ConfigOutput {
    host: Option<String>,
    username: Option<String>,
    tls_verify: bool,
    config_file: String,
    credentials_file: String,
}

pub fn run(config: &Config, json: bool) -> Result<()> {
    let config_file = Config::path()?.display().to_string();
    let credentials_file = Config::credentials_path()?.display().to_string();
    let tls_verify = !config.insecure.unwrap_or(false);

    if json {
        let output = ConfigOutput {
            host: config.host.clone(),
            username: config.username.clone(),
            tls_verify,
            config_file,
            credentials_file,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print_kv(&[
            (
                "host".to_string(),
                config
                    .host
                    .clone()
                    .unwrap_or_else(|| "(not set)".to_string()),
            ),
            (
                "username".to_string(),
                config
                    .username
                    .clone()
                    .unwrap_or_else(|| "(not set)".to_string()),
            ),
            ("tls_verify".to_string(), tls_verify.to_string()),
            ("config_file".to_string(), config_file),
            ("credentials_file".to_string(), credentials_file),
        ]);
    }

    Ok(())
}
