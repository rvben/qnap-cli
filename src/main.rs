mod client;
mod commands;
mod config;
mod output;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use client::QnapClient;
use config::{Config, normalize_host_input, read_password_from_stdin};
use output::OutputFormat;

#[derive(Debug, Parser)]
#[command(name = "qnap", about = "QNAP NAS management CLI", version)]
struct Cli {
    /// Override the NAS host for this command (always HTTPS)
    #[arg(long, global = true)]
    host: Option<String>,

    /// Override the NAS username for this command
    #[arg(long, short = 'u', global = true)]
    username: Option<String>,

    /// Skip TLS certificate verification for this command
    #[arg(long, global = true, conflicts_with = "secure")]
    insecure: bool,

    /// Re-enable TLS certificate verification for this command
    #[arg(long, global = true, conflicts_with = "insecure")]
    secure: bool,

    /// Read the password from stdin instead of the saved credentials file or prompt
    #[arg(long, global = true)]
    password_stdin: bool,

    /// Output format (auto, text, json)
    #[arg(long, short = 'o', global = true, default_value = "auto")]
    output: OutputFormat,

    /// Machine-readable JSON output (alias for --output json)
    #[arg(long, global = true, hide = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

impl Cli {
    fn effective_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.output
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Save credentials and verify connectivity
    Login,

    /// Show system information (model, firmware, hostname, uptime)
    Info,

    /// Show system resource usage (CPU, RAM, temperature)
    Status,

    /// List storage volumes and disks
    Volumes,

    /// List shared folders
    Shares {
        /// Maximum number of shares to return
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Number of shares to skip
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Comma-separated list of fields to include in JSON output
        #[arg(long)]
        fields: Option<String>,
    },

    /// File operations
    Files {
        #[command(subcommand)]
        action: FilesCommand,
    },

    /// Save raw API responses for debugging and compatibility reporting
    Dump {
        /// Directory to write response files into (created if it does not exist)
        #[arg(default_value = "./qnap-dump")]
        dir: String,
    },

    /// Show network adapter information
    Network,

    /// Show current saved configuration
    Config,

    /// Print command schema for agent use
    Schema,

    /// Generate shell completion script
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
enum FilesCommand {
    /// List files and directories at PATH
    Ls {
        /// Remote path (e.g. /Public)
        path: String,

        /// Fetch all results, paginating past the 200-item default limit
        #[arg(long)]
        all: bool,

        /// Recursively list all files in subdirectories
        #[arg(long, short = 'r')]
        recursive: bool,

        /// Maximum number of items to return
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Number of items to skip
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Comma-separated list of fields to include in JSON output
        #[arg(long)]
        fields: Option<String>,
    },

    /// Show metadata for a file or directory
    Stat {
        /// Remote path
        path: String,
    },

    /// Create a directory
    Mkdir {
        /// Remote path to create (e.g. /Public/newdir)
        path: String,

        /// Confirm without prompting (required when not in a TTY)
        #[arg(long)]
        yes: bool,
    },

    /// Delete one or more files or directories
    Rm {
        /// Remote paths to delete
        #[arg(required = true)]
        paths: Vec<String>,

        /// Confirm without prompting (required when not in a TTY)
        #[arg(long)]
        yes: bool,
    },

    /// Move or rename a file or directory
    Mv {
        /// Source remote path
        src: String,
        /// Destination remote path
        dst: String,

        /// Confirm without prompting (required when not in a TTY)
        #[arg(long)]
        yes: bool,
    },

    /// Copy a file or directory
    Cp {
        /// Source remote path
        src: String,
        /// Destination remote path
        dst: String,
        /// Overwrite if destination exists
        #[arg(long)]
        overwrite: bool,

        /// Confirm without prompting (required when not in a TTY)
        #[arg(long)]
        yes: bool,
    },

    /// Upload a local file or directory to the NAS
    Upload {
        /// Local file or directory to upload
        local: std::path::PathBuf,
        /// Remote directory to upload into (e.g. /Public)
        remote_dir: String,
        /// Overwrite if file already exists
        #[arg(long)]
        overwrite: bool,
        /// Recursively upload a directory and its contents
        #[arg(long, short = 'r')]
        recursive: bool,

        /// Confirm without prompting (required when not in a TTY)
        #[arg(long)]
        yes: bool,
    },

    /// Download a file or directory from the NAS
    Download {
        /// Remote file or directory path (e.g. /Public/photos)
        remote: String,
        /// Local path to save to (defaults to name in current directory)
        local: Option<std::path::PathBuf>,
        /// Recursively download a directory and its contents
        #[arg(long, short = 'r')]
        recursive: bool,
    },

    /// Search for files matching a pattern
    Find {
        /// Remote path to search under (e.g. /Public)
        path: String,
        /// Glob pattern to match filenames (e.g. "*.txt", "backup*")
        pattern: String,
    },
}

fn password_override(password_stdin: bool) -> Result<Option<String>> {
    if password_stdin {
        return Ok(Some(read_password_from_stdin()?));
    }
    Ok(None)
}

fn apply_runtime_overrides(mut config: Config, cli: &Cli) -> Result<Config> {
    if let Some(host) = &cli.host {
        config.host = Some(normalize_host_input(host)?);
    }

    if let Some(username) = &cli.username {
        let username = username.trim();
        if username.is_empty() {
            bail!("username must not be empty");
        }
        config.username = Some(username.to_string());
    }

    if cli.insecure {
        config.insecure = Some(true);
    } else if cli.secure {
        config.insecure = Some(false);
    }

    Ok(config)
}

async fn authenticated_client(
    config: &Config,
    password_override: Option<&str>,
) -> Result<QnapClient> {
    let mut client = QnapClient::new(config)?;
    let password = match password_override {
        Some(password) => password.to_string(),
        None => config.password()?,
    };
    client.login(&config.username()?, &password).await?;
    Ok(client)
}

/// Map an error to a kind string and exit code for scripting.
fn error_kind_and_code(err: &anyhow::Error) -> (&'static str, i32) {
    let msg = format!("{:#}", err);
    if msg.contains("failed to reach NAS") || msg.contains("error sending request") {
        return ("network_error", 5);
    }
    if msg.contains("authentication failed")
        || msg.contains("authPassed")
        || msg.contains("Invalid login")
    {
        return ("auth", 4);
    }
    if msg.contains("permission denied") {
        return ("permission_denied", 3);
    }
    if msg.contains("path not found") || msg.contains("not found") {
        return ("not_found", 2);
    }
    ("general", 1)
}

/// Require explicit confirmation for destructive operations when not in a TTY.
///
/// When stdin is not a terminal and --yes was not passed, emits a structured
/// error envelope and exits with code 2.
fn require_confirmation(operation: &str, yes: bool) {
    use std::io::IsTerminal;
    if yes || std::io::stdin().is_terminal() {
        return;
    }
    let envelope = serde_json::json!({
        "error": {
            "kind": "confirmation_required",
            "message": format!("{} requires confirmation", operation),
            "hint": "Re-run with --yes to confirm."
        }
    });
    eprintln!("{}", envelope);
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        let (kind, code) = error_kind_and_code(&err);
        let envelope = serde_json::json!({
            "error": {
                "kind": kind,
                "message": format!("{:#}", err)
            }
        });
        eprintln!("{}", envelope);
        std::process::exit(code);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::try_parse().unwrap_or_else(|clap_err| {
        // Emit help/version messages to stdout (clap convention) but wrap
        // real parse errors as a structured envelope on stderr so piped
        // consumers get a machine-readable signal instead of prose.
        use clap::error::ErrorKind;
        match clap_err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                // Let clap print help/version normally then exit cleanly.
                clap_err.exit();
            }
            _ => {
                let msg = clap_err.to_string();
                // Strip ANSI escapes from clap's colored error text.
                let clean: String = {
                    let mut s = String::with_capacity(msg.len());
                    let mut in_escape = false;
                    for ch in msg.chars() {
                        if ch == '\x1b' {
                            in_escape = true;
                        } else if in_escape {
                            if ch == 'm' {
                                in_escape = false;
                            }
                        } else {
                            s.push(ch);
                        }
                    }
                    s
                };
                let envelope = serde_json::json!({
                    "error": {
                        "kind": "general",
                        "message": clean.trim()
                    }
                });
                eprintln!("{}", envelope);
                std::process::exit(clap_err.exit_code());
            }
        }
    });
    let fmt = cli.effective_format();

    match &cli.command {
        Command::Login => {
            commands::login::run(
                cli.host.clone(),
                cli.username.clone(),
                cli.insecure,
                cli.secure,
                cli.password_stdin,
            )
            .await?;
        }

        Command::Schema => {
            commands::schema::run();
        }

        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), "qnap", &mut std::io::stdout());
        }

        Command::Config => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            commands::config_show::run(&config, fmt)?;
        }

        Command::Network => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::network::run(&client, fmt).await?;
        }

        Command::Info => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::info::run(&client, fmt).await?;
        }

        Command::Status => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::status::run(&client, fmt).await?;
        }

        Command::Volumes => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::volumes::run(&client, fmt).await?;
        }

        Command::Shares {
            limit,
            offset,
            fields,
        } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::shares::run(&client, fmt, *limit, *offset, fields.as_deref()).await?;
        }

        Command::Dump { dir } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::dump::run(&client, std::path::Path::new(dir)).await?;
        }

        Command::Files { action } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            match action {
                FilesCommand::Ls {
                    path,
                    all,
                    recursive,
                    limit,
                    offset,
                    fields,
                } => {
                    if *recursive {
                        commands::files::list_recursive(&client, path, fmt).await?;
                    } else {
                        commands::files::list(
                            &client,
                            path,
                            *all,
                            fmt,
                            *limit,
                            *offset,
                            fields.as_deref(),
                        )
                        .await?;
                    }
                }
                FilesCommand::Stat { path } => {
                    commands::files::stat(&client, path, fmt).await?;
                }
                FilesCommand::Mkdir { path, yes } => {
                    require_confirmation("files mkdir", *yes);
                    commands::files::mkdir(&client, path).await?;
                }
                FilesCommand::Rm { paths, yes } => {
                    require_confirmation("files rm", *yes);
                    for path in paths {
                        commands::files::rm(&client, path).await?;
                    }
                }
                FilesCommand::Mv { src, dst, yes } => {
                    require_confirmation("files mv", *yes);
                    commands::files::mv(&client, src, dst).await?;
                }
                FilesCommand::Cp {
                    src,
                    dst,
                    overwrite,
                    yes,
                } => {
                    require_confirmation("files cp", *yes);
                    commands::files::cp(&client, src, dst, *overwrite).await?;
                }
                FilesCommand::Upload {
                    local,
                    remote_dir,
                    overwrite,
                    recursive,
                    yes,
                } => {
                    require_confirmation("files upload", *yes);
                    if *recursive {
                        commands::files::upload_recursive(&client, local, remote_dir, *overwrite)
                            .await?;
                    } else {
                        commands::files::upload(&client, local, remote_dir, *overwrite).await?;
                    }
                }
                FilesCommand::Download {
                    remote,
                    local,
                    recursive,
                } => {
                    if *recursive {
                        let local_path = local.clone().unwrap_or_else(|| {
                            let name = remote
                                .trim_end_matches('/')
                                .rsplit('/')
                                .next()
                                .unwrap_or("download");
                            std::path::PathBuf::from(name)
                        });
                        commands::files::download_recursive(&client, remote, &local_path).await?;
                    } else {
                        commands::files::download(&client, remote, local.as_deref()).await?;
                    }
                }
                FilesCommand::Find { path, pattern } => {
                    commands::files::find(&client, path, pattern, fmt).await?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::{Parser, error::ErrorKind};

    #[test]
    fn files_ls_requires_a_path_argument() {
        let err = Cli::try_parse_from(["qnap", "files", "ls"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn login_tls_flags_conflict() {
        let err = Cli::try_parse_from(["qnap", "login", "--insecure", "--secure"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn global_password_stdin_flag_parses_before_subcommand() {
        Cli::try_parse_from(["qnap", "--password-stdin", "info"]).unwrap();
    }

    #[test]
    fn global_host_override_parses_after_subcommand() {
        Cli::try_parse_from(["qnap", "info", "--host", "nas.local"]).unwrap();
    }

    #[test]
    fn files_rm_requires_at_least_one_path() {
        let err = Cli::try_parse_from(["qnap", "files", "rm"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn files_rm_accepts_multiple_paths() {
        Cli::try_parse_from(["qnap", "files", "rm", "/Public/a.txt", "/Public/b.txt"]).unwrap();
    }

    #[test]
    fn files_ls_recursive_flag_parses() {
        Cli::try_parse_from(["qnap", "files", "ls", "-r", "/Public"]).unwrap();
    }

    #[test]
    fn completions_parses_known_shell() {
        Cli::try_parse_from(["qnap", "completions", "zsh"]).unwrap();
        Cli::try_parse_from(["qnap", "completions", "bash"]).unwrap();
        Cli::try_parse_from(["qnap", "completions", "fish"]).unwrap();
    }

    #[test]
    fn output_flag_parses_auto() {
        let cli = Cli::try_parse_from(["qnap", "--output", "auto", "info"]).unwrap();
        assert_eq!(cli.output, super::OutputFormat::Auto);
    }

    #[test]
    fn output_flag_parses_json() {
        let cli = Cli::try_parse_from(["qnap", "--output", "json", "info"]).unwrap();
        assert_eq!(cli.output, super::OutputFormat::Json);
    }

    #[test]
    fn output_short_flag_parses() {
        let cli = Cli::try_parse_from(["qnap", "-o", "text", "info"]).unwrap();
        assert_eq!(cli.output, super::OutputFormat::Text);
    }

    #[test]
    fn hidden_json_flag_sets_effective_format_to_json() {
        let cli = Cli::try_parse_from(["qnap", "--json", "info"]).unwrap();
        assert_eq!(cli.effective_format(), super::OutputFormat::Json);
    }

    #[test]
    fn files_yes_flag_parses_for_rm() {
        Cli::try_parse_from(["qnap", "files", "rm", "--yes", "/Public/a.txt"]).unwrap();
    }

    #[test]
    fn files_yes_flag_parses_for_mkdir() {
        Cli::try_parse_from(["qnap", "files", "mkdir", "--yes", "/Public/newdir"]).unwrap();
    }

    #[test]
    fn shares_limit_offset_parse() {
        let cli =
            Cli::try_parse_from(["qnap", "shares", "--limit", "50", "--offset", "10"]).unwrap();
        if let super::Command::Shares { limit, offset, .. } = cli.command {
            assert_eq!(limit, 50);
            assert_eq!(offset, 10);
        } else {
            panic!("wrong command variant");
        }
    }

    #[test]
    fn error_kind_network_error() {
        let err = anyhow::anyhow!("failed to reach NAS: error sending request");
        let (kind, code) = super::error_kind_and_code(&err);
        assert_eq!(kind, "network_error");
        assert_eq!(code, 5);
    }

    #[test]
    fn error_kind_permission_denied() {
        let err = anyhow::anyhow!("rm: permission denied: /Public/locked");
        let (kind, code) = super::error_kind_and_code(&err);
        assert_eq!(kind, "permission_denied");
        assert_eq!(code, 3);
    }

    #[test]
    fn error_kind_not_found() {
        let err = anyhow::anyhow!("path not found: /Public/missing.txt");
        let (kind, code) = super::error_kind_and_code(&err);
        assert_eq!(kind, "not_found");
        assert_eq!(code, 2);
    }

    #[test]
    fn error_kind_general() {
        let err = anyhow::anyhow!("something unexpected happened");
        let (kind, code) = super::error_kind_and_code(&err);
        assert_eq!(kind, "general");
        assert_eq!(code, 1);
    }
}
