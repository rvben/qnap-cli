use anyhow::Result;

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
        config.host = Some(h);
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
        config.host = Some(prompt("Host (e.g. 192.168.1.50)")?);
    }
    if config.username.is_none() {
        config.username = Some(prompt("Username")?);
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

fn prompt(label: &str) -> Result<String> {
    use std::io::{self, Write};
    print!("{}: ", label);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_password(label: &str) -> Result<String> {
    rpassword::prompt_password(format!("{}: ", label)).map_err(Into::into)
}
