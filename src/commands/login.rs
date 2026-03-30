use anyhow::{Result, bail};

use crate::client::QnapClient;
use crate::config::{Config, normalize_host_input, read_password_from_stdin};

pub async fn run(
    host: Option<String>,
    username: Option<String>,
    insecure: bool,
    secure: bool,
    password_stdin: bool,
) -> Result<()> {
    let mut config = Config::load()?;

    if let Some(host) = host {
        config.host = Some(normalize_host_input(&host)?);
    }
    if let Some(username) = username {
        let username = username.trim();
        if username.is_empty() {
            bail!("username must not be empty");
        }
        config.username = Some(username.to_string());
    }
    if insecure {
        config.insecure = Some(true);
    } else if secure {
        config.insecure = Some(false);
    }

    if config.host.is_none() {
        config.host = Some(prompt_host()?);
    }
    if config.username.is_none() {
        config.username = Some(prompt_required("Username")?);
    }

    let password = resolve_password(password_stdin)?;

    let mut client = QnapClient::new(&config)?;
    client.login(config.username()?.as_str(), &password).await?;

    config.save()?;
    Config::save_password(
        config.host()?.as_str(),
        config.username()?.as_str(),
        &password,
    )?;

    let config_path = Config::path()?;
    let credentials_path = Config::credentials_path()?;
    println!("Logged in successfully.");
    println!("  Config:   {}", config_path.display());
    println!("  Password: saved to {}", credentials_path.display());
    Ok(())
}

fn prompt_host() -> Result<String> {
    loop {
        let input = prompt_required("Host (e.g. nas.local or https://nas.local)")?;
        match normalize_host_input(&input) {
            Ok(host) => return Ok(host),
            Err(err) => eprintln!("  ({})", err),
        }
    }
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

fn resolve_password(password_stdin: bool) -> Result<String> {
    if password_stdin {
        return read_password_from_stdin();
    }

    if let Ok(password) = std::env::var("QNAP_PASSWORD")
        && !password.is_empty()
    {
        return Ok(password);
    }

    prompt_password("Password")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_host_input_accepts_ip() {
        assert_eq!(
            normalize_host_input("192.168.1.50").unwrap(),
            "https://192.168.1.50"
        );
    }

    #[test]
    fn test_normalize_host_input_accepts_https_url() {
        assert_eq!(
            normalize_host_input("https://nas.local").unwrap(),
            "https://nas.local"
        );
    }

    #[test]
    fn test_normalize_host_input_trims_whitespace() {
        assert_eq!(
            normalize_host_input("  nas.local  ").unwrap(),
            "https://nas.local"
        );
    }

    #[test]
    fn test_normalize_host_input_accepts_host_with_port() {
        assert_eq!(
            normalize_host_input("nas.local:8080").unwrap(),
            "https://nas.local:8080"
        );
    }

    #[test]
    fn test_normalize_host_input_rejects_http() {
        assert!(normalize_host_input("http://nas.local").is_err());
    }

    #[test]
    fn test_normalize_host_input_rejects_empty() {
        assert!(normalize_host_input("").is_err());
    }
}
