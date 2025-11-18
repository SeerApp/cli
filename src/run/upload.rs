use reqwest::{Client, Response};
use tokio::fs;
use std::path::PathBuf;

pub async fn upload_file(
    client: &Client,
    server_url: &str,
    path: &PathBuf,
    token: &String,
) -> anyhow::Result<Response> {
    let url = format!("{}/upload", server_url);

    tracing::info!("Uploading {} -> {}", path.display(), url);

    let bytes = fs::read(path).await?;

    let res = client
        .post(url)
        .bearer_auth(token)
        .body(bytes)
        .send()
        .await?;

    if !res.status().is_success() {
        anyhow::bail!("Upload failed with status {}", res.status());
    }

    Ok(res)
}
