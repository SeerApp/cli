pub fn normalize_sha(s: &str) -> String {
    let trimmed = s.strip_prefix("0x").unwrap_or(s);
    trimmed.to_ascii_lowercase()
}