use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
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
        toml::from_str(&content).context("failed to parse config")
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

    pub fn host(&self) -> Result<&str> {
        self.host
            .as_deref()
            .context("no host configured — run `qnap login` first")
    }

    pub fn username(&self) -> Result<&str> {
        self.username
            .as_deref()
            .context("no username configured — run `qnap login` first")
    }

    pub fn password(&self) -> Result<&str> {
        self.password
            .as_deref()
            .context("no password configured — run `qnap login` first")
    }
}
