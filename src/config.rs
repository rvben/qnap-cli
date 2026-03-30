use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

pub fn normalize_host_input(host: &str) -> Result<String> {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("host must not be empty");
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    };

    let url =
        reqwest::Url::parse(&candidate).with_context(|| format!("invalid host '{}'", trimmed))?;

    match url.scheme() {
        "https" => {}
        "http" => bail!("plaintext HTTP is not supported; use HTTPS for QNAP connections"),
        scheme => bail!("unsupported scheme '{}' ; use HTTPS", scheme),
    }

    let host = url
        .host_str()
        .context("host must include a hostname or IP address")?
        .to_ascii_lowercase();

    if url.query().is_some() || url.fragment().is_some() {
        bail!("host must not include query strings or fragments");
    }

    if url.path() != "/" && !url.path().is_empty() {
        bail!("host must not include a path; pass only the NAS host or origin");
    }

    let mut normalized = if host.contains(':') && !host.starts_with('[') {
        format!("https://[{}]", host)
    } else {
        format!("https://{}", host)
    };
    if let Some(port) = url.port()
        && Some(port) != default_port_for_scheme("https")
    {
        normalized.push(':');
        normalized.push_str(&port.to_string());
    }

    Ok(normalized)
}

fn canonical_host_scope(host: &str) -> String {
    if let Ok(normalized) = normalize_host_input(host)
        && let Ok(url) = reqwest::Url::parse(&normalized)
        && let Some(raw_host) = url.host_str()
    {
        let host = raw_host.to_ascii_lowercase();
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{}]", host)
        } else {
            host
        };

        return match url.port() {
            Some(port) if Some(port) != default_port_for_scheme(url.scheme()) => {
                format!("{}:{}", host, port)
            }
            _ => host,
        };
    }

    host.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Superset of Config used only during deserialization to detect and scrub legacy fields.
#[derive(Debug, Deserialize, Default)]
struct LegacyConfig {
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub insecure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub host: Option<String>,
    pub username: Option<String>,
    pub insecure: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredCredentials {
    host: String,
    username: String,
    password: String,
}

impl StoredCredentials {
    fn matches(&self, host: &str, username: &str) -> bool {
        self.host == canonical_host_scope(host) && self.username == username
    }
}

fn ensure_parent_dir(path: &Path, private_dir: bool) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("file {} must have a parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create directory {}", parent.display()))?;

    let metadata = std::fs::metadata(parent)
        .with_context(|| format!("failed to stat {}", parent.display()))?;
    if !metadata.is_dir() {
        bail!("{} is not a directory", parent.display());
    }

    #[cfg(unix)]
    if private_dir {
        let permissions = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(parent, permissions).with_context(|| {
            format!("failed to set private permissions on {}", parent.display())
        })?;
    }

    Ok(parent.to_path_buf())
}

fn ensure_regular_file_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                bail!("credentials file {} must not be a symlink", path.display());
            }
            if !file_type.is_file() {
                bail!("credentials file {} must be a regular file", path.display());
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }

    Ok(())
}

fn new_temp_path(path: &Path, attempt: u32) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("file {} must have a parent directory", path.display()))?;
    let name = path
        .file_name()
        .with_context(|| format!("file {} must have a name", path.display()))?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".tmp-{}-{}-{}", std::process::id(), nonce, attempt));

    Ok(parent.join(temp_name))
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> Result<()> {
    std::fs::File::open(path)
        .with_context(|| format!("failed to open directory {}", path.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync directory {}", path.display()))
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> Result<()> {
    Ok(())
}

fn write_atomic_file(
    path: &Path,
    content: &str,
    file_mode: Option<u32>,
    private_dir: bool,
) -> Result<()> {
    let parent = ensure_parent_dir(path, private_dir)?;
    ensure_regular_file_path(path)?;

    for attempt in 0..32 {
        let candidate = new_temp_path(path, attempt)?;
        let mut open_options = std::fs::OpenOptions::new();
        open_options.create_new(true).write(true);
        #[cfg(unix)]
        if let Some(mode) = file_mode {
            open_options.mode(mode);
        }

        match open_options.open(&candidate) {
            Ok(mut file) => {
                #[cfg(unix)]
                if let Some(mode) = file_mode {
                    let permissions = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&candidate, permissions).with_context(|| {
                        format!(
                            "failed to set secure permissions on {}",
                            candidate.display()
                        )
                    })?;
                }

                file.write_all(content.as_bytes())
                    .with_context(|| format!("failed to write {}", candidate.display()))?;
                file.sync_all()
                    .with_context(|| format!("failed to sync {}", candidate.display()))?;
                drop(file);

                #[cfg(windows)]
                if path.exists() {
                    std::fs::remove_file(path).with_context(|| {
                        format!("failed to replace existing file {}", path.display())
                    })?;
                }

                let rename_result = std::fs::rename(&candidate, path)
                    .with_context(|| format!("failed to atomically replace {}", path.display()));
                if rename_result.is_err() {
                    let _ = std::fs::remove_file(&candidate);
                }
                rename_result?;

                sync_dir(&parent)?;
                return Ok(());
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to create a temporary file in {}", parent.display())
                });
            }
        }
    }

    bail!(
        "failed to allocate a temporary file in {}",
        parent.display()
    );
}

fn write_secure_file(path: &Path, content: &str) -> Result<()> {
    write_atomic_file(path, content, Some(0o600), true)
}

fn write_config_file(path: &Path, content: &str) -> Result<()> {
    write_atomic_file(path, content, Some(0o600), true)
}

fn ensure_private_credentials_file(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat credentials file {}", path.display()))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        bail!("credentials file {} must not be a symlink", path.display());
    }
    if !file_type.is_file() {
        bail!("credentials file {} must be a regular file", path.display());
    }

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            bail!(
                "credentials file {} must not be accessible by group or others; run `chmod 600 {}`",
                path.display(),
                path.display()
            );
        }
    }
    Ok(())
}

fn trim_password_input(input: &str) -> String {
    input.trim_end_matches(&['\r', '\n'][..]).to_string()
}

fn parse_bool_env(var: &str) -> Result<Option<bool>> {
    match std::env::var(var) {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            if value.is_empty() {
                return Ok(None);
            }

            match value.as_str() {
                "1" | "true" | "yes" => Ok(Some(true)),
                "0" | "false" | "no" => Ok(Some(false)),
                _ => bail!("{} must be one of: 1, true, yes, 0, false, no", var),
            }
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", var)),
    }
}

pub fn read_password_from_stdin() -> Result<String> {
    use std::io::Read;

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let password = trim_password_input(&input);
    if password.is_empty() {
        bail!("password from stdin must not be empty");
    }
    Ok(password)
}

impl Config {
    fn dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("could not find config directory")?
            .join("qnap-cli");
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.toml"))
    }

    pub fn credentials_path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("credentials.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;

        let legacy: LegacyConfig = toml::from_str(&content).context("failed to parse config")?;
        let config = Config {
            host: legacy.host,
            username: legacy.username,
            insecure: legacy.insecure,
        };

        if legacy.password.is_some() {
            let _ = config.save();
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = toml::to_string_pretty(self)?;
        write_config_file(&path, &content)
            .with_context(|| format!("failed to write config to {}", path.display()))
    }

    /// Resolve host: QNAP_HOST env var → config file → error.
    pub fn host(&self) -> Result<String> {
        if let Ok(host) = std::env::var("QNAP_HOST")
            && !host.trim().is_empty()
        {
            return normalize_host_input(&host);
        }

        let host = self
            .host
            .clone()
            .context("no host configured — run `qnap login` first")?;
        normalize_host_input(&host)
    }

    /// Resolve username: QNAP_USERNAME env var → config file → error.
    pub fn username(&self) -> Result<String> {
        if let Ok(username) = std::env::var("QNAP_USERNAME")
            && !username.trim().is_empty()
        {
            return Ok(username.trim().to_string());
        }

        let username = self
            .username
            .clone()
            .context("no username configured — run `qnap login` first")?;
        let username = username.trim();
        if username.is_empty() {
            bail!("username is empty — run `qnap login` again");
        }
        Ok(username.to_string())
    }

    /// Resolve password: QNAP_PASSWORD env var → credentials file → error.
    pub fn password(&self) -> Result<String> {
        if let Ok(password) = std::env::var("QNAP_PASSWORD")
            && !password.is_empty()
        {
            return Ok(password);
        }

        let host = self.host()?;
        let username = self.username()?;
        let host_label = canonical_host_scope(&host);

        let Some(credentials) = Self::load_credentials()? else {
            bail!(
                "no saved password for '{}' on '{}' — run `qnap login` first or set `QNAP_PASSWORD`",
                username,
                host_label
            );
        };

        if credentials.matches(&host, &username) {
            if credentials.password.is_empty() {
                bail!(
                    "saved password for '{}' on '{}' is empty — run `qnap login` again",
                    username,
                    host_label
                );
            }
            return Ok(credentials.password);
        }

        bail!(
            "saved password does not match '{}' on '{}' — run `qnap login` again or set `QNAP_PASSWORD`",
            username,
            host_label
        );
    }

    /// Resolve insecure flag: QNAP_INSECURE env var → config file → false.
    pub fn insecure(&self) -> Result<bool> {
        if let Some(value) = parse_bool_env("QNAP_INSECURE")? {
            return Ok(value);
        }
        Ok(self.insecure.unwrap_or(false))
    }

    pub fn save_password(host: &str, username: &str, password: &str) -> Result<()> {
        let path = Self::credentials_path()?;
        save_credentials(&path, host, username, password)
    }

    fn load_credentials() -> Result<Option<StoredCredentials>> {
        let path = Self::credentials_path()?;
        load_credentials_from_path(&path)
    }
}

fn save_credentials(path: &Path, host: &str, username: &str, password: &str) -> Result<()> {
    let credentials = StoredCredentials {
        host: canonical_host_scope(host),
        username: username.to_string(),
        password: password.to_string(),
    };
    let content = toml::to_string_pretty(&credentials)?;
    write_secure_file(path, &content)
}

fn load_credentials_from_path(path: &Path) -> Result<Option<StoredCredentials>> {
    if !path.exists() {
        return Ok(None);
    }

    ensure_private_credentials_file(path)?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read credentials at {}", path.display()))?;
    let credentials: StoredCredentials =
        toml::from_str(&content).context("failed to parse credentials file")?;
    Ok(Some(credentials))
}

#[cfg(test)]
mod tests {
    use super::{
        Config, StoredCredentials, canonical_host_scope, load_credentials_from_path,
        normalize_host_input, parse_bool_env, save_credentials, trim_password_input,
    };
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn normalize_host_input_defaults_to_https() {
        assert_eq!(
            normalize_host_input("NAS.LOCAL").unwrap(),
            "https://nas.local"
        );
    }

    #[test]
    fn normalize_host_input_preserves_non_default_port() {
        assert_eq!(
            normalize_host_input("https://nas.local:8080/").unwrap(),
            "https://nas.local:8080"
        );
    }

    #[test]
    fn normalize_host_input_rejects_http() {
        let err = normalize_host_input("http://nas.local").unwrap_err();
        assert!(
            err.to_string().contains("plaintext HTTP is not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn normalize_host_input_rejects_paths() {
        let err = normalize_host_input("https://nas.local/admin").unwrap_err();
        assert!(err.to_string().contains("must not include a path"));
    }

    #[test]
    fn canonical_host_scope_normalizes_case_and_scheme() {
        assert_eq!(canonical_host_scope("NAS.LOCAL"), "nas.local");
        assert_eq!(canonical_host_scope("https://NAS.LOCAL/"), "nas.local");
    }

    #[test]
    fn canonical_host_scope_keeps_non_default_ports() {
        assert_eq!(canonical_host_scope("nas.local:8080"), "nas.local:8080");
        assert_eq!(
            canonical_host_scope("https://NAS.LOCAL:8080"),
            "nas.local:8080"
        );
    }

    #[test]
    fn canonical_host_scope_drops_default_https_port() {
        assert_eq!(canonical_host_scope("https://nas.local:443/"), "nas.local");
    }

    #[test]
    fn stored_credentials_match_requires_same_host_and_username() {
        let credentials = StoredCredentials {
            host: "nas.local".to_string(),
            username: "admin".to_string(),
            password: "secret".to_string(),
        };

        assert!(credentials.matches("https://nas.local/", "admin"));
        assert!(!credentials.matches("https://other.local/", "admin"));
        assert!(!credentials.matches("https://nas.local/", "guest"));
    }

    #[test]
    fn trim_password_input_removes_line_endings_only() {
        assert_eq!(trim_password_input("secret\n"), "secret");
        assert_eq!(trim_password_input("secret\r\n"), "secret");
        assert_eq!(trim_password_input("secret"), "secret");
    }

    #[test]
    fn parse_bool_env_accepts_strict_values() {
        unsafe {
            std::env::set_var("QNAP_INSECURE", "true");
        }
        assert_eq!(parse_bool_env("QNAP_INSECURE").unwrap(), Some(true));

        unsafe {
            std::env::set_var("QNAP_INSECURE", "0");
        }
        assert_eq!(parse_bool_env("QNAP_INSECURE").unwrap(), Some(false));

        unsafe {
            std::env::remove_var("QNAP_INSECURE");
        }
    }

    #[test]
    fn parse_bool_env_rejects_ambiguous_values() {
        unsafe {
            std::env::set_var("QNAP_INSECURE", "maybe");
        }
        let err = parse_bool_env("QNAP_INSECURE").unwrap_err();
        assert!(err.to_string().contains("must be one of"));
        unsafe {
            std::env::remove_var("QNAP_INSECURE");
        }
    }

    #[test]
    fn config_host_normalizes_env_override() {
        unsafe {
            std::env::set_var("QNAP_HOST", "NAS.LOCAL");
        }
        let config = Config::default();
        assert_eq!(config.host().unwrap(), "https://nas.local");
        unsafe {
            std::env::remove_var("QNAP_HOST");
        }
    }

    #[test]
    fn save_credentials_writes_private_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("credentials.toml");

        save_credentials(&path, "https://nas.local", "admin", "secret").unwrap();

        let credentials = load_credentials_from_path(&path).unwrap().unwrap();
        assert_eq!(credentials.host, "nas.local");
        assert_eq!(credentials.username, "admin");
        assert_eq!(credentials.password, "secret");

        #[cfg(unix)]
        {
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);

            let dir_mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
    }

    #[test]
    fn save_credentials_overwrites_without_leaking_temp_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("credentials.toml");

        save_credentials(&path, "https://nas.local", "admin", "old-secret").unwrap();
        save_credentials(&path, "https://nas.local", "admin", "new-secret").unwrap();

        let credentials = load_credentials_from_path(&path).unwrap().unwrap();
        assert_eq!(credentials.password, "new-secret");

        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() != "credentials.toml")
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn save_credentials_rejects_non_regular_targets() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("credentials.toml");
        std::fs::create_dir(&path).unwrap();

        let err = save_credentials(&path, "https://nas.local", "admin", "secret").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn save_credentials_rejects_symlink_targets() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.toml");
        std::fs::write(&target, "password = 'leave-me-alone'\n").unwrap();
        let path = dir.path().join("credentials.toml");
        symlink(&target, &path).unwrap();

        let err = save_credentials(&path, "https://nas.local", "admin", "secret").unwrap_err();
        assert!(err.to_string().contains("must not be a symlink"));
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "password = 'leave-me-alone'\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_credentials_rejects_symlink_targets() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.toml");
        std::fs::write(
            &target,
            "host = 'nas.local'\nusername = 'admin'\npassword = 'secret'\n",
        )
        .unwrap();
        let path = dir.path().join("credentials.toml");
        symlink(&target, &path).unwrap();

        let err = load_credentials_from_path(&path).unwrap_err();
        assert!(err.to_string().contains("must not be a symlink"));
    }
}
