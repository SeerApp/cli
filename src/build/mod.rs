use clap::Parser;
use anyhow::Result;

mod build;
mod debug_check;
mod debug_flag;
mod project;

#[derive(Parser, Debug)]
pub struct BuildArgs {
    #[arg(long, hide = true)]
    pub cleanup_seer: bool,
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

        println!("\nCreating seer.toml for each program...");
        let mut seer_toml_paths = Vec::new();
        for prog in &programs {
            let seer_toml_path = debug_flag::create_seer_toml(&prog.manifest_path, args.silent)?;
            seer_toml_paths.push((prog.name.clone(), seer_toml_path));
        }

        println!("\nBuilding programs with DWARF debug info using seer.toml...");
        let build_results = if args.silent {
            build::build_all_programs_silent(&programs, &seer_toml_paths)
        } else {
            build::build_all_programs(&programs, &seer_toml_paths)
        };
        let (_, failed_debugs_raw) = debug_check::check_all_debug_files(&programs, args.silent);
        let failed_debugs = debug_check::collect_failed_debug_infos(&failed_debugs_raw);

        println!("\nCleaning up seer.toml files...");
        for prog in &programs {
            debug_flag::cleanup_seer_toml(&prog.manifest_path, args.silent)?;
        }
        if args.cleanup_seer {
            println!("Cleaning up *.seer files...");
            build::cleanup_seer_files();
        }

        build::print_build_summary(&build_results, &failed_debugs);
    }

    Ok(())
}
