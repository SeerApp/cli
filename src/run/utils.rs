use anyhow::{Result, anyhow};
use base64::Engine;
use solana_sdk::transaction::VersionedTransaction;

pub fn normalize_sha(s: &str) -> String {
    let trimmed = s.strip_prefix("0x").unwrap_or(s);
    trimmed.to_ascii_lowercase()
}

pub fn parse_transaction_base64<T>(b64: &str) -> Result<VersionedTransaction>
where
    T: serde::de::DeserializeOwned + Into<VersionedTransaction>,
{
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| anyhow!("Input is not valid base64 transaction data"))?;

    let tx: T =
        bincode::deserialize(&raw).map_err(|e| anyhow!("Not a valid Solana transaction: {e}"))?;

    let versioned = tx.into();

    if versioned.signatures.is_empty() {
        return Err(anyhow!("Transaction has no signatures"));
    }

    Ok(versioned)
}
