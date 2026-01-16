use std::{
    io::{self, Write},
    path::PathBuf,
};

pub fn ask_for_consent(files: &Vec<&PathBuf>) -> bool {
    println!("You are about to upload the following files to Seer:\n");

    let mut sorted_files = files.clone();
    sorted_files.sort_by(|a, b| a.display().to_string().cmp(&b.display().to_string()));

    for p in sorted_files {
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        println!("  {}  ({})", p.display(), format_size(size));
    }

    println!(
        "\nSeer stores uploaded files temporarily and deletes them automatically after 7 days."
    );
    loop {
        print!("Do you consent? (yes/no): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => return true,
            "no" | "n" => return false,
            _ => {
                println!("Incorrect input. Please enter 'yes', 'y', 'no', or 'n'.");
            }
        }
    }
}

/// Formats size into human readable form (B, KB, MB, GB)
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;

    if b < KB {
        format!("{:.0} B", b)
    } else if b < MB {
        format!("{:.2} KB", b / KB)
    } else if b < GB {
        format!("{:.2} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}
