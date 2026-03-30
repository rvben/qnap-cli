mod client;
mod commands;
mod config;
mod output;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

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

    /// Show network adapter information
    Network {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show current saved configuration
    Config {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

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

/// Map an error to a specific exit code for scripting.
///
/// 1 = general error, 2 = not found, 3 = permission denied,
/// 4 = authentication failure, 5 = network/connection error.
fn exit_code(err: &anyhow::Error) -> i32 {
    let msg = format!("{:#}", err);
    if msg.contains("failed to reach NAS") || msg.contains("error sending request") {
        return 5;
    }
    if msg.contains("authentication failed")
        || msg.contains("authPassed")
        || msg.contains("Invalid login")
    {
        return 4;
    }
    if msg.contains("permission denied") {
        return 3;
    }
    if msg.contains("path not found") || msg.contains("not found") {
        return 2;
    }
    1
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {:#}", err);
        std::process::exit(exit_code(&err));
    }
}

async fn run() -> Result<()> {
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

        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), "qnap", &mut std::io::stdout());
        }

        Command::Config { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            commands::config_show::run(&config, *json)?;
        }

        Command::Network { json } => {
            let config = apply_runtime_overrides(Config::load()?, &cli)?;
            let password = password_override(cli.password_stdin)?;
            let client = authenticated_client(&config, password.as_deref()).await?;
            commands::network::run(&client, *json).await?;
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
                FilesCommand::Ls {
                    path,
                    all,
                    recursive,
                    json,
                } => {
                    if *recursive {
                        commands::files::list_recursive(&client, path, *json).await?;
                    } else {
                        commands::files::list(&client, path, *all, *json).await?;
                    }
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
                    recursive,
                } => {
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
    fn exit_code_network_error() {
        let err = anyhow::anyhow!("failed to reach NAS: error sending request");
        assert_eq!(super::exit_code(&err), 5);
    }

    #[test]
    fn exit_code_permission_denied() {
        let err = anyhow::anyhow!("rm: permission denied: /Public/locked");
        assert_eq!(super::exit_code(&err), 3);
    }

    #[test]
    fn exit_code_not_found() {
        let err = anyhow::anyhow!("path not found: /Public/missing.txt");
        assert_eq!(super::exit_code(&err), 2);
    }

    #[test]
    fn exit_code_general_error() {
        let err = anyhow::anyhow!("something unexpected happened");
        assert_eq!(super::exit_code(&err), 1);
    }
}
