mod build;
mod install;
mod run;
mod temp_file;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build(args) => build(args),
        Commands::Run(args) => run(args),
        Commands::Install => {
            install::install_binary().map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        },
        Commands::Login { api_key } => {
            run::auth::login_command(api_key)?;
            Ok(())
        }
    }
}
