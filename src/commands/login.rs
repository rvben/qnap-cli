use anyhow::{Result, bail};
use dialoguer::{Confirm, Input, Password};
use owo_colors::OwoColorize;

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

    // Apply CLI overrides
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

    // Interactive prompts for missing values
    if config.host.is_none() {
        config.host = Some(prompt_host(None)?);
    } else if !password_stdin {
        // Pre-fill existing host and let user confirm or change it
        let current = config.host.clone().unwrap();
        config.host = Some(prompt_host(Some(&current))?);
    }

    if config.username.is_none() {
        config.username = Some(prompt_username(None)?);
    } else if !password_stdin {
        let current = config.username.clone().unwrap();
        config.username = Some(prompt_username(Some(&current))?);
    }

    if config.insecure.is_none() && !password_stdin {
        let verify_tls = Confirm::new()
            .with_prompt("Verify TLS certificate?")
            .default(true)
            .interact()
            .map_err(|e| anyhow::anyhow!("input error: {e}"))?;
        config.insecure = Some(!verify_tls);
    }

    let password = resolve_password(password_stdin)?;

    // Validate credentials with a test API call
    let host_display = config.host.as_deref().unwrap_or("unknown");
    eprintln!("Connecting to {} ...", host_display.bold());

    let mut client = QnapClient::new(&config)?;
    match client.login(config.username()?.as_str(), &password).await {
        Ok(()) => {
            eprintln!("  {} Authentication successful", "\u{2713}".green().bold());
        }
        Err(err) => {
            eprintln!(
                "  {} Authentication failed: {}",
                "\u{2717}".red().bold(),
                err
            );
            bail!("login failed — check your credentials and try again");
        }
    }

    // Save config and credentials
    config.save()?;
    Config::save_password(
        config.host()?.as_str(),
        config.username()?.as_str(),
        &password,
    )?;

    let config_path = Config::path()?;
    eprintln!();
    eprintln!(
        "  {} Configuration saved to {}",
        "\u{2713}".green().bold(),
        config_path.display()
    );
    eprintln!();
    eprintln!("  {}:", "Next steps".bold());
    eprintln!("    qnap info               # show NAS information");
    eprintln!("    qnap volumes             # list storage volumes");
    eprintln!("    qnap completions zsh     # shell completions");

    Ok(())
}

fn prompt_host(existing: Option<&str>) -> Result<String> {
    loop {
        let prompt = Input::<String>::new()
            .with_prompt("QNAP NAS URL (e.g. nas.local or https://nas.local:8443)");

        let prompt = match existing {
            Some(val) => prompt.with_initial_text(val.to_string()),
            None => prompt,
        };

        let input = prompt
            .interact_text()
            .map_err(|e| anyhow::anyhow!("input error: {e}"))?;

        let trimmed = input.trim();
        if trimmed.is_empty() {
            eprintln!("  (URL is required)");
            continue;
        }

        match normalize_host_input(trimmed) {
            Ok(host) => return Ok(host),
            Err(err) => eprintln!("  ({})", err),
        }
    }
}

fn prompt_username(existing: Option<&str>) -> Result<String> {
    let default = existing.unwrap_or("admin").to_string();

    let value: String = Input::new()
        .with_prompt("Username")
        .default(default)
        .interact_text()
        .map_err(|e| anyhow::anyhow!("input error: {e}"))?;

    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        bail!("username must not be empty");
    }
    Ok(trimmed)
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

    let password = Password::new()
        .with_prompt("Password")
        .interact()
        .map_err(|e| anyhow::anyhow!("input error: {e}"))?;

    if password.is_empty() {
        bail!("password must not be empty");
    }

    Ok(password)
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
