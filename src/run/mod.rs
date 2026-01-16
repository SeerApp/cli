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
    #[arg(long, default_value = "./target/deploy")]
    pub artifacts: PathBuf,

    #[arg(long, default_value = "http://localhost:4770")]
    pub server_url: String,
    
    /// Skip building the project before running
    #[arg(long, default_value_t = false)]
    pub skip_build: bool,

    /// Skip consent prompt before uploading files
    #[arg(long, default_value_t = false)]
    pub consent: bool,

    /// Run build in silent mode (no output)
    #[arg(long, default_value_t = true)]
    pub silent: bool,

    /// Cleanup seer artifacts before build
    #[arg(long, default_value_t = true)]
    pub cleanup_seer: bool,
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


    let token = load_api_key()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cwd = std::env::current_dir()?;

    println!("");
    let (artifacts_dir, _artifacts_reason) = if args.artifacts == PathBuf::from("./target/deploy") {
        match crate::run::artifacts::detect_artifacts_dir(&cwd) {
            Ok(path) => {
                println!("Using autodetected artifacts directory: {} (default value was not overridden)", path.display());
                (path, "autodetected (default value)".to_string())
            },
            Err(e) => anyhow::bail!("Could not auto-detect artifacts directory: {}", e),
        }
    } else {
        println!("Using user-provided artifacts directory: {}", args.artifacts.display());
        (args.artifacts.clone(), "provided by user".to_string())
    };

    let targets: Vec<ProgramTarget> = get_targets(artifacts_dir.clone())?;
    if targets.is_empty() {
        anyhow::bail!("No valid program targets found in {:?}. Ensure .so, .debug, and -keypair.json files exist and are valid.", artifacts_dir);
    }

    // Prepare proto Session and SessionArtifact
    let mut artifacts = Vec::new();
    let mut file_map = HashMap::new(); 
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
        let so_hash = make_blob(&target.so_path)?;
        let so_size = std::fs::metadata(&target.so_path)?.len();
        let so_rel = rel(&target.so_path);
        artifacts.push(SessionArtifact {
            file_path: so_rel.to_string_lossy().to_string(),
            file_hash: so_hash.clone(),
            file_size: so_size,
        });
        file_map.insert(so_hash.clone(), (so_rel.clone(), so_size));

        // .debug
        let debug_hash = make_blob(&target.debug_path)?;
        let debug_size = std::fs::metadata(&target.debug_path)?.len();
        let debug_rel = rel(&target.debug_path);
        artifacts.push(SessionArtifact {
            file_path: debug_rel.to_string_lossy().to_string(),
            file_hash: debug_hash.clone(),
            file_size: debug_size,
        });
        file_map.insert(debug_hash.clone(), (debug_rel.clone(), debug_size));

        // -keypair.json
        let keypair_path = args.artifacts.join(format!("{}-keypair.json", target.name));
        let keypair_hash = make_blob(&keypair_path)?;
        let keypair_size = std::fs::metadata(&keypair_path)?.len();
        let keypair_rel = rel(&keypair_path);
        artifacts.push(SessionArtifact {
            file_path: keypair_rel.to_string_lossy().to_string(),
            file_hash: keypair_hash.clone(),
            file_size: keypair_size,
        });
        file_map.insert(keypair_hash.clone(), (keypair_rel.clone(), keypair_size));

        // .rs source files from debug
        match extract_source_paths(&target.debug_path, &cwd) {
            Ok(paths) => {
                for path in &paths {
                    if path.extension().and_then(|e| e.to_str()) == Some("rs") && path.exists() {
                        let src_hash = make_blob(&path)?;
                        let src_size = std::fs::metadata(&path)?.len();
                        let src_rel = rel(path);
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
        println!("");
        println!("All required files are already present on the server. No uploads needed.");
        if !create_resp.upload_info.is_empty() {
            let mut has_paths = false;
            for upload_info in &create_resp.upload_info {
                if !upload_info.file_path.is_empty() {
                    has_paths = true;
                    break;
                }
            }
            if has_paths {
                println!("Server requests upload for the following file_path values:");
                for upload_info in &create_resp.upload_info {
                    if !upload_info.file_path.is_empty() {
                        println!("  - {}", upload_info.file_path);
                    }
                }
                println!("");
            }
        }
        println!("We have the following artifact file_path values:");
        for artifact in &artifacts {
            println!("  - {}", artifact.file_path);
        }
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


        for (upload_info, path) in &missing_uploads {
            upload_file(upload_info, path).await?;
        }
    }

    println!("");
    let run_resp = client.run_session(Request::new(RunSessionRequest {})).await?.into_inner();
    println!("New Seer session at: {}", run_resp.solana_validator_url);
    Ok(())
}
