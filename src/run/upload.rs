use std::path::PathBuf;
use seer_protos_community_neoeinstein_prost::seer::sessions::v1::*;
use s3_presign::{upload as s3_upload, PresignedPost};

/// Upload a file to S3 using the presigned POST info from UploadInfo, via s3-presign crate
pub async fn upload_file(upload_info: &UploadInfo, path: &PathBuf) -> anyhow::Result<()> {
    let post = upload_info.post.as_ref().expect("Missing post upload info");
    let presigned = PresignedPost {
        url: post.url.clone(),
        fields: post.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    };

    if presigned.url.contains("fake-upload") {
        println!("[seer][mock-upload] Skipping actual upload for file: {} (path: {})", path.file_name().unwrap_or_default().to_string_lossy(), path.display());
        return Ok(());
    }

    tracing::info!("Uploading {} -> {}", path.display(), presigned.url);
    s3_upload(&presigned, path.to_str().expect("Non-UTF8 path"))
        .await?;
    Ok(())
}
