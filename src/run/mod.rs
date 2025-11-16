mod auth;
mod consent;
mod artifacts;
mod blobs;
mod source_paths;
mod utils;
mod upload;

use clap::Parser;
use serde::{Deserialize, Serialize};
use solana_sdk::transaction::VersionedTransaction;
use std::{collections::HashMap, path::PathBuf};
use tracing_subscriber::EnvFilter;

use crate::run::consent::ask_for_consent;
use crate::run::artifacts::get_targets;
use crate::run::blobs::make_blob;
use crate::run::source_paths::extract_source_paths;
use crate::run::upload::upload_file;
use crate::run::{auth::log_in, artifacts::ProgramTarget};

#[derive(Debug, Deserialize, Serialize)]
pub struct HandshakeResponse {
    pub missing_blobs: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Debug)]
pub struct FinalProgramTarget {
    pub name: String,
    pub so_blob: String,
    pub debug_blob: String,
    pub sources: HashMap<PathBuf, String>,
    pub pubkey: String,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[arg(allow_hyphen_values = true)]
    pub tx: String,

    #[arg(long, default_value = "./target/deploy")]
    pub artifacts: PathBuf,

    #[arg(long, default_value = "http://127.0.0.1:3000")]
    pub server_url: String,

    #[arg(long, default_value = "https://zhtktqshwmxnyfvthtzx.supabase.co")]
    pub supabase_url: String,

    #[arg(long, default_value = "sb_publishable_47h9YcsXtA_RDVmUmarLKA_zyqM2YbF")]
    pub anon_key: String,
}

#[tokio::main]
pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    let session = log_in(&args.supabase_url, &args.anon_key).await?;
    let token = session.access_token;

    utils::parse_transaction_base64::<VersionedTransaction>(&args.tx)?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cwd = std::env::current_dir()?;

    let targets: Vec<ProgramTarget> =
        get_targets(args.artifacts.clone()).expect("Failed to read artifacts.");

    let mut source_files: Vec<PathBuf> = Vec::new();
    let mut blob_to_prog: HashMap<String, PathBuf> = HashMap::new();
    let mut name_to_source: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let mut program_to_source_blobs: HashMap<String, HashMap<PathBuf, String>> = HashMap::new();

    for target in &targets {
        let debug_blob = make_blob(&target.debug_path)?;

        match extract_source_paths(&target.debug_path, &cwd) {
            Ok(paths) => {
                let mut source_to_blobs: HashMap<PathBuf, String> = HashMap::new();
                for path in &paths {
                    source_to_blobs.insert(path.clone(), make_blob(&path)?);
                }
                program_to_source_blobs.insert(target.name.clone(), source_to_blobs);
                source_files.extend(paths.clone());
                name_to_source.insert(target.name.clone(), paths);
            }
            Err(err) => {
                eprintln!(
                    "Failed to extract source paths for {:?}: {:?}",
                    target.debug_path, err
                );
            }
        }

        blob_to_prog.insert(make_blob(&target.so_path)?, target.so_path.clone());
        blob_to_prog.insert(debug_blob, target.debug_path.clone());
    }

    let mut blob_to_source: HashMap<String, PathBuf> = HashMap::new();
    let mut source_to_blob: HashMap<PathBuf, String> = HashMap::new();

    for sf in source_files {
        let blob = make_blob(&sf)?;
        blob_to_source.insert(blob.clone(), sf.clone());
        source_to_blob.insert(sf.clone(), blob);
    }

    let mut all_blobs: Vec<String> = blob_to_source.clone().into_keys().collect();
    let prog_blobs: Vec<String> = blob_to_prog.clone().into_keys().collect();
    all_blobs.extend(prog_blobs);

    let client = reqwest::Client::new();

    tracing::info!("Sending handshake to {}", args.server_url);

    let handshake_response: HandshakeResponse = client
        .post(format!("{}/handshake", args.server_url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "blobs": all_blobs
        }))
        .send()
        .await?
        .json::<HandshakeResponse>()
        .await?;

    let mut missing_paths = Vec::new();
    for mb in handshake_response.missing_blobs {
        if blob_to_source.contains_key(&mb) {
            missing_paths.push(blob_to_source.get(&mb).unwrap());
        } else if blob_to_prog.contains_key(&mb) {
            missing_paths.push(blob_to_prog.get(&mb).unwrap());
        } else {
            anyhow::bail!("Server returned blob CLI did not request!");
        }
    }

    if missing_paths.len() > 0 {
        let consent = ask_for_consent(&missing_paths);

        if !consent {
            return Ok(());
        }

        for mp in missing_paths {
            upload_file(&client, &args.server_url, mp, &token).await?;
        }
    }

    let mut programs: Vec<FinalProgramTarget> = Vec::new();
    for pt in targets {
        programs.push(FinalProgramTarget {
            name: pt.name.clone(),
            so_blob: make_blob(&pt.so_path)?,
            debug_blob: make_blob(&pt.debug_path)?,
            pubkey: pt.pubkey,
            sources: program_to_source_blobs.get(&pt.name).unwrap().clone(),
        });
    }

    let resp = client
        .post(format!("{}/run", args.server_url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "cwd": cwd,
            "programs": programs,
            "tx": args.tx,
        }))
        .send()
        .await?;

    if resp.status().is_success() {
        let ok = resp.json::<RunResponse>().await?;
        println!("{}", ok.message);
    } else {
        let err = resp.json::<ErrorResponse>().await?;
        eprintln!("Server responded with error: {}", err.error);
    }

    Ok(())
}
