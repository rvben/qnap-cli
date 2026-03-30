use anyhow::{bail, Result};

use crate::client::QnapClient;
use crate::config::Config;

pub async fn run(
    host: Option<String>,
    username: Option<String>,
    password: Option<String>,
    insecure: bool,
) -> Result<()> {
    let mut config = Config::load()?;

    if let Some(h) = host {
        config.host = Some(validated_host(&h)?);
    }
    if let Some(u) = username {
        config.username = Some(u);
    }
    if let Some(p) = password {
        config.password = Some(p);
    }
    if insecure {
        config.insecure = Some(true);
    }

    // Prompt for missing credentials
    if config.host.is_none() {
        config.host = Some(prompt_required("Host (e.g. 192.168.1.50)")?);
    }
    if config.username.is_none() {
        config.username = Some(prompt_required("Username")?);
    }
    if config.password.is_none() {
        config.password = Some(prompt_password("Password")?);
    }

    // Test authentication
    let mut client = QnapClient::new(&config)?;
    let sid = client
        .login(config.username()?, config.password()?)
        .await?;

    config.save()?;
    println!("Logged in successfully (sid={}...)", &sid[..sid.len().min(8)]);
    Ok(())
}

fn validated_host(host: &str) -> Result<String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        bail!("host must not be empty");
    }
    Ok(trimmed.to_string())
}

fn prompt_required(label: &str) -> Result<String> {
    use std::io::{self, Write};
    loop {
        print!("{}: ", label);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let value = input.trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
        eprintln!("  (value required, please try again)");
    }
}

fn prompt_password(label: &str) -> Result<String> {
    rpassword::prompt_password(format!("{}: ", label)).map_err(Into::into)
}
