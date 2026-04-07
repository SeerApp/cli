mod build;
mod install;
mod run;
mod temp_file;
mod update;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    build::{build, BuildArgs},
    run::{run, RunArgs},
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build programs for Seer
    Build(BuildArgs),

    /// Trace a transaction  
    Run(RunArgs),

    /// Install the Seer binary globally 
    Install,

    /// Log in with your Seer API key 
    Login {
        api_key: Option<String>,
    },

    /// Update Seer CLI to the latest version
    Update {
        /// Skip confirmation prompt
        #[arg(long, default_value_t = false)]
        yes: bool,

        /// Install a specific version (e.g. 0.2.1 or v0.2.1)
        #[arg(long)]
        version: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build(args) => {
            let rt = tokio::runtime::Runtime::new()?;
            let _ = rt.block_on(update::maybe_notify_update());
            build(args)
        }
        Commands::Run(args) => {
            let rt = tokio::runtime::Runtime::new()?;
            let _ = rt.block_on(update::maybe_notify_update());
            run(args)
        }
        Commands::Install => {
            install::install_binary().map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        },
        Commands::Login { api_key } => {
            let rt = tokio::runtime::Runtime::new()?;
            let _ = rt.block_on(update::maybe_notify_update());
            run::auth::login_command(api_key)?;
            Ok(())
        }
        Commands::Update { yes, version } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(update::run_update_command(yes, version))?;
            Ok(())
        }
    }
}
