use std::env;
use std::fs;
use std::path::PathBuf;

/// Installs the seer-cli binary to a directory in the user's PATH.
/// On Windows, installs to %USERPROFILE%\.cargo\bin.
/// On Unix, installs to /usr/local/bin.
pub fn install_binary() -> std::io::Result<()> {
    let exe = env::current_exe()?;
    #[cfg(target_os = "windows")]
    let target_dir = dirs::home_dir().unwrap().join(".cargo").join("bin");
    #[cfg(not(target_os = "windows"))]
    let target_dir = PathBuf::from("/usr/local/bin");
    fs::create_dir_all(&target_dir)?;
    #[cfg(target_os = "windows")]
    let target_path = target_dir.join("seer.exe");
    #[cfg(not(target_os = "windows"))]
    let target_path = target_dir.join("seer");
    fs::copy(&exe, &target_path)?;
    println!("seer installed to {}", target_path.display());
    Ok(())
}
