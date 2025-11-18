use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    pub access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
    user: serde_json::Value,
}

pub async fn log_in(supabase_url: &str, anon_key: &str) -> anyhow::Result<Session> {
    let proj = ProjectDirs::from("com", "seer", "cli")
        .expect("Unable to determine config directory for Seer.");

    let session_path: PathBuf = proj.config_dir().join("session.json");

    fs::create_dir_all(proj.config_dir())?;

    if session_path.exists() {
        if let Ok(existing) = read_session(&session_path).await {
            if validate_session(&existing, supabase_url, anon_key).await {
                return Ok(existing);
            }
        }
    }

    println!("🔐  Seer authentication required.");
    print!("Email: ");
    io::Write::flush(&mut io::stdout()).unwrap();

    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    let password = rpassword::prompt_password("Password: ")?;

    let session = request_new_session(&email, &password, supabase_url, anon_key).await?;

    let json = serde_json::to_string_pretty(&session)?;
    fs::write(&session_path, json)?;

    println!("✅ Logged in as {email}");

    Ok(session)
}

async fn read_session(path: &PathBuf) -> anyhow::Result<Session> {
    let contents = fs::read_to_string(path)?;
    let session: Session = serde_json::from_str(&contents)?;
    Ok(session)
}

async fn validate_session(session: &Session, supabase_url: &str, anon_key: &str) -> bool {
    let url = format!("{}/auth/v1/user", supabase_url);

    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .header("apikey", anon_key)
        .bearer_auth(&session.access_token)
        .send()
        .await;

    match res {
        Ok(r) => r.status().is_success(),
        _ => false,
    }
}

async fn request_new_session(
    email: &str,
    password: &str,
    supabase_url: &str,
    anon_key: &str,
) -> anyhow::Result<Session> {
    let url = format!("{}/auth/v1/token?grant_type=password", supabase_url);

    let client = reqwest::Client::new();

    let resp = client
        .post(url)
        .header("apikey", anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": email,
            "password": password
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Login failed: {}", resp.text().await?);
    }

    let session = resp.json::<Session>().await?;
    Ok(session)
}
