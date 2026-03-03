use directories::ProjectDirs;
use std::fs;
use std::io::{self, Write};
use rpassword::read_password;

const API_KEY_FILENAME: &str = "api_key";

pub fn store_api_key(api_key: &str) -> anyhow::Result<()> {
    let proj = ProjectDirs::from("com", "seer", "seer")
        .expect("Unable to determine config directory for Seer.");
    let config_dir = proj.config_dir().join("cli");
    fs::create_dir_all(&config_dir)?;
    let key_path = config_dir.join(API_KEY_FILENAME);
    fs::write(&key_path, api_key.trim())?;
    println!("✅ API key saved to {}", key_path.display());
    Ok(())
}

pub fn load_api_key() -> anyhow::Result<String> {
    // 1. Check SEER_API_KEY environment variable first
    if let Ok(key) = std::env::var("SEER_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // 2. Fall back to config file
    let proj = ProjectDirs::from("com", "seer", "seer")
        .expect("Unable to determine config directory for Seer.");
    let config_dir = proj.config_dir().join("cli");
    let key_path = config_dir.join(API_KEY_FILENAME);
    let api_key = fs::read_to_string(&key_path)
        .map_err(|_| anyhow::anyhow!("API key not found. Please set the SEER_API_KEY environment variable or run 'seer login [api key]'."))?;
    Ok(api_key.trim().to_string())
}

pub fn login_command(api_key: Option<String>) -> anyhow::Result<()> {
    match api_key {
        Some(key) => store_api_key(&key),
        None => {
            print!("Enter your Seer API key: ");
            io::stdout().flush()?;
            let key = read_password()?;
            store_api_key(&key)
        }
    }
}

