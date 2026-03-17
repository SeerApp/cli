use anyhow::Result;
use clap::Parser;

mod build;
mod debug_check;
mod debug_flag;
mod project;

#[derive(Parser, Debug)]
pub struct BuildArgs {
    #[arg(long, default_value_t = true, hide = true, action = clap::ArgAction::Set)]
    pub cleanup_seer: bool,

    /// Build programs silently.
    #[arg(long)]
    pub silent: bool,

    /// Force build even if Solana CLI version is below v3.
    #[arg(long)]
    pub force: bool,
}

pub fn build(args: BuildArgs) -> Result<()> {
    // Check Solana CLI version
    match project::get_solana_cli_major_version() {
        Ok(major) if major < 3 => {
            if !args.force {
                eprintln!("WARNING: Solana CLI version is below v3. Seer requires Solana CLI v3 or higher to work correctly.");
                eprintln!("         Your build may produce broken artifacts.");
                eprintln!("         Update your Solana CLI or use --force to build anyway.");
                anyhow::bail!("Solana CLI version too low.");
            } else {
                eprintln!("WARNING: Solana CLI version is below v3. This is a broken build — Solana CLI must be updated for Seer to work correctly.");
            }
        }
        Ok(_) => {} // v3 or higher, all good
        Err(e) => {
            eprintln!("WARNING: Could not determine Solana CLI version: {}", e);
            if !args.force {
                eprintln!("         Use --force to build anyway.");
                anyhow::bail!("Build aborted.");
            }
        }
    }

    let programs = project::detect_solana_programs()?;
    if programs.is_empty() {
        println!("No Solana native programs detected in this project.");
    } else {
        println!("Detected Solana native programs:");
        for prog in &programs {
            println!("- {} ({})", prog.name, prog.manifest_path.display());
        }

        let mut seer_toml_paths = Vec::new();
        for prog in &programs {
            seer_toml_paths.push((
                prog.name.clone(),
                debug_flag::create_seer_toml(args.cleanup_seer, &prog.manifest_path)?,
            ));
        }

        println!("\nBuilding programs...");
        let build_results = if args.silent {
            build::build_all_programs_silent(&programs, &seer_toml_paths)
        } else {
            build::build_all_programs(&programs, &seer_toml_paths)
        };
        let (_, failed_debugs_raw) = debug_check::check_all_debug_files(&programs, args.silent);
        let failed_debugs = debug_check::collect_failed_debug_infos(&failed_debugs_raw);

        build::print_build_summary(&build_results, &failed_debugs);
    }

    Ok(())
}


