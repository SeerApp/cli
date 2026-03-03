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
}

pub fn build(args: BuildArgs) -> Result<()> {
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
                debug_flag::create_seer_toml(args.cleanup_seer, &prog.manifest_path)?
                    .path()
                    .clone(),
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
