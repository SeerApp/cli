use clap::{Parser, Subcommand};
mod build;
mod install;
use anyhow::Result;

use crate::build::{BuildArgs, build};

/// CLI tool to build Solana native programs with DWARF debug info

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build(BuildArgs),
    Install,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build(args) => {
            build(args)
        }
        Commands::Install => {
            install::install_binary().map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        }
    }
}
