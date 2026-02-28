pub mod auth;
mod consent;
mod artifacts;
mod blobs;
mod source_paths;
mod utils;
mod upload;


use seer_protos_community_neoeinstein_prost::seer::sessions::v1::*;
use seer_protos_community_neoeinstein_tonic::seer::sessions::v1::tonic::sessions_service_client::SessionsServiceClient;
use tonic::{Request, transport::Channel, metadata::MetadataValue};
use clap::Parser;
use std::{collections::HashMap, path::PathBuf};
use tracing_subscriber::EnvFilter;
use crate::run::consent::ask_for_consent;
use crate::run::artifacts::get_targets;
use crate::run::blobs::make_blob;
use crate::run::source_paths::extract_source_paths;
use crate::run::upload::upload_file;
use crate::run::{auth::load_api_key, artifacts::ProgramTarget};

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Customize path to the build artifacts directory.
    #[arg(long, default_value = "./target/deploy")]
    pub artifacts: PathBuf,

    #[arg(long, default_value = "http://localhost:4770", hide = true)]
    pub server_url: String,

    /// Skip building programs before uploading.
    #[arg(long, default_value_t = false)]
    pub skip_build: bool,

    /// Automatically approve uploading and temporary storage of files by Seer.
    #[arg(long, default_value_t = false)]
    pub consent: bool,

    /// Build programs silently.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub silent: bool,

    #[arg(long, default_value_t = true, hide = true, action = clap::ArgAction::Set)]
    pub cleanup_seer: bool,

    /// API key to use for this run (overrides environment variable and config file).
    #[arg(long, value_name = "API_KEY", help = "API key to use for this run (overrides env/config)")]
    pub api_key: Option<String>,
}



#[tokio::main]
pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    if !args.skip_build {
        let build_args = crate::build::BuildArgs {
            cleanup_seer: args.cleanup_seer,
            silent: args.silent,
        };
        crate::build::build(build_args)?;
    }


    // Use --api-key if provided, else fallback to env/config
    let token = if let Some(ref key) = args.api_key {
        key.trim().to_string()
    } else {
        load_api_key()?
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cwd = std::env::current_dir()?;

    println!("");
    let artifacts_dir = if args.artifacts == PathBuf::from("./target/deploy") {
        let deploy_dir = cwd.join("target/deploy");
        if deploy_dir.is_dir() {
            println!("Using autodetected artifacts directory: {} (default value was not overridden)", deploy_dir.display());
            deploy_dir
        } else {
            anyhow::bail!(
                "Could not find build artifacts directory: {}\nExpected artifacts at: {}\nIf you use a custom build location, use the --artifacts flag.",
                cwd.display(),
                deploy_dir.display()
            );
        }
    } else {
        println!("Using user-provided artifacts directory: {}", args.artifacts.display());
        println!("Note: Build artifacts (.so, .debug, -keypair.json) must be generated only through seer build to work correctly. If you provide a custom directory, ensure it contains valid seer build outputs.");
        args.artifacts.clone()
    };

    let targets: Vec<ProgramTarget> = get_targets(artifacts_dir.clone())?;
    if targets.is_empty() {
        anyhow::bail!("No valid program targets found in {:?}. Ensure .so, .debug, and -keypair.json files exist and are valid.", artifacts_dir);
    }

    // Prepare proto Session and SessionArtifact
    let mut artifacts = Vec::new();
    let mut file_map = HashMap::new(); 
    let mut files_to_send = Vec::new();
    for target in &targets {
        let rel = |p: &PathBuf| {
            let rel_path = p.strip_prefix(&cwd).unwrap_or(p).to_path_buf();
            let rel_str = rel_path.to_string_lossy();
            if rel_str.starts_with("./") || rel_str.starts_with("../") {
                rel_path
            } else {
                PathBuf::from(format!("./{}", rel_str))
            }
        };

        // .so
        crate::run::artifacts::process_artifact(
            &target.so_path,
            &rel,
            &mut files_to_send,
            &mut artifacts,
            &mut file_map
        )?;

        // .debug
        crate::run::artifacts::process_artifact(
            &target.debug_path,
            &rel,
            &mut files_to_send,
            &mut artifacts,
            &mut file_map
        )?;

        // -keypair.json
        crate::run::artifacts::process_artifact(
            &target.json_path,
            &rel,
            &mut files_to_send,
            &mut artifacts,
            &mut file_map
        )?;

        // .rs source files from debug
        match extract_source_paths(&target.debug_path, &cwd) {
            Ok(paths) => {
                for path in &paths {
                    if path.extension().and_then(|e| e.to_str()) == Some("rs") && path.exists() {
                        let src_hash = make_blob(&path)?;
                        let src_size = std::fs::metadata(&path)?.len();
                        let src_rel = rel(path);
                        files_to_send.push(src_rel.to_string_lossy().to_string());
                        artifacts.push(SessionArtifact {
                            file_path: src_rel.to_string_lossy().to_string(),
                            file_hash: src_hash.clone(),
                            file_size: src_size,
                        });
                        file_map.insert(src_hash.clone(), (src_rel.clone(), src_size));
                    } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                        println!("[seer][warn] .rs source file listed in debug info but not found on disk: {}", path.display());
                    }
                }
            }
            Err(err) => {
                eprintln!("Failed to extract source paths for {:?}: {:?}", target.debug_path, err);
            }
        }
    }

    // gRPC: connect and set up client with auth
    let channel = Channel::from_shared(args.server_url.clone())?.connect().await?;
    let token_val: MetadataValue<_> = format!("Bearer {}", token).parse()?;
    let mut client = SessionsServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
        req.metadata_mut().insert("authorization", token_val.clone());
        Ok(req)
    });

    let create_req = CreateSessionRequest {
        session: Some(Session {
            project_path: cwd.to_string_lossy().to_string(),
            artifacts: artifacts.clone(),
        }),
    };
    let create_resp = client.create_session(Request::new(create_req)).await?.into_inner();

    let mut missing_uploads = Vec::new();
    for upload_info in &create_resp.upload_info {
        let hash = artifacts.iter().find(|a| a.file_path == upload_info.file_path).map(|a| a.file_hash.clone());
        if let Some(hash) = hash {
            if let Some((path, _size)) = file_map.get(&hash) {
                missing_uploads.push((upload_info, path));
            }
        }
    }

    if missing_uploads.is_empty() {
        println!("\nAll required files are already present on the server. No uploads needed.");
    } 
    else {
        let missing_paths: Vec<&PathBuf> = missing_uploads.iter().map(|(_, path)| *path).collect();
        let consent = if args.consent {
            true
        } else {
            ask_for_consent(&missing_paths)
        };
        if !consent {
            return Ok(());
        }


        let upload_futures = missing_uploads.iter().map(|(info, path)| {
            upload_file(info, path)
        });
        let results = futures::future::join_all(upload_futures).await;
        for result in results {
            result?;
        }
    }

    println!("");
    let run_resp = client.run_session(Request::new(RunSessionRequest {})).await?.into_inner();
    println!("New Seer session at: {}", run_resp.solana_validator_url);
    Ok(())
}
