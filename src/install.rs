use std::env;
use std::fs;
use std::path::PathBuf;

/// Installs the seer-cli binary to a directory in the user's PATH.
/// Unix/Linux/macOS:
/// - Tries $HOME/.local/bin if writable
/// - Falls back to /usr/local/bin if writable
/// - Otherwise uses $HOME/.local/bin and creates it
/// Windows:
/// - Uses `<home>\.cargo\bin`
pub fn install_binary() -> std::io::Result<()> {
    let exe = env::current_exe()?;
    
    let target_dir = if cfg!(target_os = "windows") {
        // Windows: use .cargo/bin in home directory
        let home = dirs::home_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine home directory",
            )
        })?;
        home.join(".cargo").join("bin")
    } else {
        // Unix/Linux/macOS
        if let Ok(home) = env::var("HOME") {
            let local_bin = PathBuf::from(&home).join(".local").join("bin");

            if is_writable(&local_bin) {
                local_bin
            } else {
                let usr_local = PathBuf::from("/usr/local/bin");
                if is_writable(&usr_local) {
                    usr_local
                } else {
                    local_bin
                }
            }
        } else {
            PathBuf::from("/usr/local/bin")
        }
    };
    
    fs::create_dir_all(&target_dir)?;
    
    #[cfg(target_os = "windows")]
    let target_path = target_dir.join("seer.exe");
    #[cfg(not(target_os = "windows"))]
    let target_path = target_dir.join("seer");
    
    fs::copy(&exe, &target_path)?;
    println!("seer installed to {}", target_path.display());
    Ok(())
}

fn is_writable(path: &PathBuf) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = metadata.permissions();
                (permissions.mode() & 0o200) != 0
            }
            #[cfg(not(unix))]
            {
                !metadata.permissions().readonly()
            }
        }
        Err(_) => false,
    }
}
