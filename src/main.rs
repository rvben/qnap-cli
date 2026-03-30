mod client;
mod commands;
mod config;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

use client::QnapClient;
use config::Config;

#[derive(Debug, Parser)]
#[command(name = "qnap", about = "QNAP NAS management CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Save credentials and verify connectivity
    Login {
        /// NAS host (e.g. 192.168.1.50 or https://nas.local)
        #[arg(long)]
        host: Option<String>,

        /// Username
        #[arg(long, short)]
        username: Option<String>,

        /// Skip TLS certificate verification
        #[arg(long)]
        insecure: bool,
    },

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

    /// Print command schema for agent use
    Schema,
}

#[derive(Debug, Subcommand)]
enum FilesCommand {
    /// List files and directories at PATH
    Ls {
        /// Remote path (e.g. /Public)
        #[arg(default_value = "/")]
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
}

async fn authenticated_client(config: &Config) -> Result<QnapClient> {
    let mut client = QnapClient::new(config)?;
    client
        .login(&config.username()?, &config.password()?)
        .await?;
    Ok(client)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Login {
            host,
            username,
            insecure,
        } => {
            commands::login::run(host, username, insecure).await?;
        }

        Command::Schema => {
            commands::schema::run();
        }

        Command::Info { json } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            commands::info::run(&client, json).await?;
        }

        Command::Status { json } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            commands::status::run(&client, json).await?;
        }

        Command::Volumes { json } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            commands::volumes::run(&client, json).await?;
        }

        Command::Shares { json } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            commands::shares::run(&client, json).await?;
        }

        Command::Dump { dir } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            commands::dump::run(&client, std::path::Path::new(&dir)).await?;
        }

        Command::Files { action } => {
            let config = Config::load()?;
            let client = authenticated_client(&config).await?;
            match action {
                FilesCommand::Ls { path, all, json } => {
                    commands::files::list(&client, &path, all, json).await?;
                }
                FilesCommand::Stat { path, json } => {
                    commands::files::stat(&client, &path, json).await?;
                }
            }
        }
    }

    Ok(())
}
