use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "qnap-cli";

/// Superset of Config used only during deserialization to detect and migrate legacy fields.
#[derive(Debug, Deserialize, Default)]
struct LegacyConfig {
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub insecure: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub host: Option<String>,
    pub username: Option<String>,
    pub insecure: Option<bool>,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("could not find config directory")?
            .join("qnap-cli");
        Ok(dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;

        // Parse via a superset that includes the legacy plaintext password field
        let legacy: LegacyConfig = toml::from_str(&content).context("failed to parse config")?;
        let config = Config {
            host: legacy.host,
            username: legacy.username,
            insecure: legacy.insecure,
        };

        // Migrate plaintext password to OS keychain and rewrite config without it
        if let (Some(username), Some(password)) = (&config.username, &legacy.password)
            && Self::save_password(username, password).is_ok()
        {
            let _ = config.save();
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("failed to write config to {}", path.display()))
    }

    /// Resolve host: QNAP_HOST env var → config file → error.
    pub fn host(&self) -> Result<String> {
        if let Ok(h) = std::env::var("QNAP_HOST")
            && !h.is_empty()
        {
            return Ok(h);
        }
        let h = self
            .host
            .clone()
            .context("no host configured — run `qnap login` first")?;
        if h.is_empty() {
            anyhow::bail!("host is empty — run `qnap login` again");
        }
        Ok(h)
    }

    /// Resolve username: QNAP_USERNAME env var → config file → error.
    pub fn username(&self) -> Result<String> {
        if let Ok(u) = std::env::var("QNAP_USERNAME")
            && !u.is_empty()
        {
            return Ok(u);
        }
        self.username
            .clone()
            .context("no username configured — run `qnap login` first")
    }

    /// Resolve password: QNAP_PASSWORD env var → OS keychain → error.
    pub fn password(&self) -> Result<String> {
        if let Ok(p) = std::env::var("QNAP_PASSWORD")
            && !p.is_empty()
        {
            return Ok(p);
        }
        let username = self.username()?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &username)
            .context("failed to access system keychain")?;
        entry.get_password().with_context(|| {
            format!(
                "no password stored for '{}' — run `qnap login` first",
                username
            )
        })
    }

    /// Resolve insecure flag: QNAP_INSECURE env var → config file → false.
    pub fn insecure(&self) -> bool {
        if let Ok(v) = std::env::var("QNAP_INSECURE")
            && !v.is_empty()
            && v != "0"
            && v != "false"
        {
            return true;
        }
        self.insecure.unwrap_or(false)
    }

    /// Store a password for the given username in the OS keychain.
    pub fn save_password(username: &str, password: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .context("failed to access system keychain")?;
        entry
            .set_password(password)
            .context("failed to save password to system keychain")
    }
}
