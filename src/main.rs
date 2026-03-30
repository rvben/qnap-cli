mod client;
mod commands;
mod config;
mod output;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use client::QnapClient;
use config::{Config, normalize_host_input, read_password_from_stdin};

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

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Save credentials and verify connectivity
    Login,

    /// Show system information (model, firmware, hostname, uptime)
    Info {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show system resource usage (CPU, RAM, temperature)
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List storage volumes and disks
    Volumes {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List shared folders
    Shares {
        /// Output as JSON
        #[arg(long)]
        json: bool,
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

    /// Show current saved configuration
    Config {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Print command schema for agent use
    Schema,
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

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show metadata for a file or directory
    Stat {
        /// Remote path
        path: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Create a directory
    Mkdir {
        /// Remote path to create (e.g. /Public/newdir)
        path: String,
    },

    /// Delete one or more files or directories
    Rm {
        /// Remote paths to delete
        #[arg(required = true)]
        paths: Vec<String>,
    },

    /// Move or rename a file or directory
    Mv {
        /// Source remote path
        src: String,
        /// Destination remote path
        dst: String,
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
    },

    /// Upload a local file to the NAS
    Upload {
        /// Local file to upload
        local: std::path::PathBuf,
        /// Remote directory to upload into (e.g. /Public)
        remote_dir: String,
        /// Overwrite if file already exists
        #[arg(long)]
        overwrite: bool,
    },

    /// Download a file from the NAS
    Download {
        /// Remote file path (e.g. /Public/file.txt)
        remote: String,
        /// Local path to save to (defaults to filename in current directory)
        local: Option<std::path::PathBuf>,
    },

    /// Search for files matching a pattern
    Find {
        /// Remote path to search under (e.g. /Public)
        path: String,
        /// Glob pattern to match filenames (e.g. "*.txt", "backup*")
        pattern: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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

        Command::Config { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            commands::config_show::run(&config, *json)?;
        }

        Command::Info { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::info::run(&client, *json).await?;
        }

        Command::Status { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::status::run(&client, *json).await?;
        }

        Command::Volumes { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::volumes::run(&client, *json).await?;
        }

        Command::Shares { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::shares::run(&client, *json).await?;
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
                FilesCommand::Ls { path, all, json } => {
                    commands::files::list(&client, path, *all, *json).await?;
                }
                FilesCommand::Stat { path, json } => {
                    commands::files::stat(&client, path, *json).await?;
                }
                FilesCommand::Mkdir { path } => {
                    commands::files::mkdir(&client, path).await?;
                }
                FilesCommand::Rm { paths } => {
                    for path in paths {
                        commands::files::rm(&client, path).await?;
                    }
                }
                FilesCommand::Mv { src, dst } => {
                    commands::files::mv(&client, src, dst).await?;
                }
                FilesCommand::Cp {
                    src,
                    dst,
                    overwrite,
                } => {
                    commands::files::cp(&client, src, dst, *overwrite).await?;
                }
                FilesCommand::Upload {
                    local,
                    remote_dir,
                    overwrite,
                } => {
                    commands::files::upload(&client, local, remote_dir, *overwrite).await?;
                }
                FilesCommand::Download { remote, local } => {
                    commands::files::download(&client, remote, local.as_deref()).await?;
                }
                FilesCommand::Find {
                    path,
                    pattern,
                    json,
                } => {
                    commands::files::find(&client, path, pattern, *json).await?;
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
}
