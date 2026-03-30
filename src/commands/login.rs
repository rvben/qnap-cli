use anyhow::{Result, bail};

use crate::client::QnapClient;
use crate::config::Config;

pub async fn run(host: Option<String>, username: Option<String>, insecure: bool) -> Result<()> {
    let mut config = Config::load()?;

    if let Some(h) = host {
        config.host = Some(validated_host(&h)?);
    }
    if let Some(u) = username {
        config.username = Some(u);
    }
    if insecure {
        config.insecure = Some(true);
    }

    // Prompt for any missing required fields
    if config.host.is_none() {
        config.host = Some(prompt_required("Host (e.g. 192.168.1.50)")?);
    }
    if config.username.is_none() {
        config.username = Some(prompt_required("Username")?);
    }

    // Always prompt for password — never stored in config
    let password = prompt_password("Password")?;

    // Verify credentials against the NAS before saving
    let mut client = QnapClient::new(&config)?;
    let sid = client.login(config.username()?.as_str(), &password).await?;

    // Persist config (without password) and save password to OS keychain
    config.save()?;
    Config::save_password(config.username()?.as_str(), &password)?;

    let config_path = Config::path()?;
    println!(
        "Logged in successfully (sid={}...)",
        &sid[..sid.len().min(8)]
    );
    println!("  Config:   {}", config_path.display());
    println!("  Password: saved to system keychain");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_host_accepts_ip() {
        assert_eq!(validated_host("192.168.1.50").unwrap(), "192.168.1.50");
    }

    #[test]
    fn test_validated_host_accepts_url() {
        assert_eq!(
            validated_host("https://nas.local").unwrap(),
            "https://nas.local"
        );
    }

    #[test]
    fn test_validated_host_trims_whitespace() {
        assert_eq!(validated_host("  192.168.1.50  ").unwrap(), "192.168.1.50");
    }

    #[test]
    fn test_validated_host_rejects_empty() {
        assert!(validated_host("").is_err());
    }

    #[test]
    fn test_validated_host_rejects_whitespace_only() {
        assert!(validated_host("   ").is_err());
    }
}
