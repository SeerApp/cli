mod build;
mod install;
mod run;

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
    Build(BuildArgs),
    Run(RunArgs),
    Install,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build(args) => build(args),
        Commands::Run(args) => run(args),
        Commands::Install => {
            install::install_binary().map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        }
    }
}
