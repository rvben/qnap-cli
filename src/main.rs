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

        /// Password (omit to be prompted)
        #[arg(long, short)]
        password: Option<String>,

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

    /// List storage volumes
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

    /// Print command schema for agent use
    Schema,
}

#[derive(Debug, Subcommand)]
enum FilesCommand {
    /// List files and directories at PATH
    Ls {
        /// Remote path (e.g. /share/Public)
        #[arg(default_value = "/")]
        path: String,

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Login {
            host,
            username,
            password,
            insecure,
        } => {
            commands::login::run(host, username, password, insecure).await?;
        }

        Command::Schema => {
            commands::schema::run();
        }

        // All other commands require an authenticated client
        cmd => {
            let config = Config::load()?;
            let mut client = QnapClient::new(&config)?;
            client
                .login(config.username()?, config.password()?)
                .await?;

            match cmd {
                Command::Info { json } => commands::info::run(&client, json).await?,
                Command::Status { json } => commands::status::run(&client, json).await?,
                Command::Volumes { json } => commands::volumes::run(&client, json).await?,
                Command::Shares { json } => commands::shares::run(&client, json).await?,
                Command::Files { action } => match action {
                    FilesCommand::Ls { path, json } => {
                        commands::files::list(&client, &path, json).await?
                    }
                    FilesCommand::Stat { path, json } => {
                        commands::files::stat(&client, &path, json).await?
                    }
                },
                Command::Login { .. } | Command::Schema => unreachable!(),
            }
        }
    }

    Ok(())
}
