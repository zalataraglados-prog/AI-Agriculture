use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Default, Serialize, Deserialize)]
struct TokenStore {
    #[serde(default)]
    hourly_token_hashes: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct TokenStoreCompat {
    #[serde(default)]
    hourly_token_hashes: HashMap<String, String>,
    #[serde(default)]
    hourly_tokens: HashMap<String, String>,
}

pub(crate) fn current_hour_token(path: &str) -> Result<String, String> {
    let key = current_hour_key();
    let token = generate_token();

    let mut store = load_store(path)?;
    store
        .hourly_token_hashes
        .insert(key.clone(), hash_secret(&token));
    prune_old_hours(&mut store.hourly_token_hashes, &key);
    save_store(path, &store)?;

    Ok(token)
}

pub(crate) fn validate_current_hour_token(path: &str, candidate: &str) -> Result<bool, String> {
    let key = current_hour_key();
    let mut store = load_store(path)?;

    if !store.hourly_token_hashes.contains_key(&key) {
        let issued = generate_token();
        store
            .hourly_token_hashes
            .insert(key.clone(), hash_secret(&issued));
        prune_old_hours(&mut store.hourly_token_hashes, &key);
        save_store(path, &store)?;
    }

    let Some(expected_hash) = store.hourly_token_hashes.get(&key) else {
        return Ok(false);
    };

    Ok(hash_secret(candidate) == expected_hash)
}

fn load_store(path: &str) -> Result<TokenStore, String> {
    ensure_parent_dir(path)?;
    if !Path::new(path).exists() {
        return Ok(TokenStore::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read token store {}: {e}", path))?;

    let compat = serde_json::from_str::<TokenStoreCompat>(&content)
        .map_err(|e| format!("Failed to parse token store {}: {e}", path))?;

    let mut store = TokenStore {
        hourly_token_hashes: compat.hourly_token_hashes,
    };

    if store.hourly_token_hashes.is_empty() && !compat.hourly_tokens.is_empty() {
        for (hour, token) in compat.hourly_tokens {
            store.hourly_token_hashes.insert(hour, hash_secret(&token));
        }
        save_store(path, &store)?;
    }

    Ok(store)
}

fn save_store(path: &str, store: &TokenStore) -> Result<(), String> {
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize token store: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Failed to write token store {}: {e}", path))
}

fn current_hour_key() -> String {
    let epoch_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (epoch_sec / 3600).to_string()
}

fn generate_token() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

fn prune_old_hours(tokens: &mut HashMap<String, String>, current_hour: &str) {
    let current = match current_hour.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return,
    };

    tokens.retain(|hour, _| match hour.parse::<u64>() {
        Ok(v) => current.saturating_sub(v) <= 72,
        Err(_) => false,
    });
}

fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn ensure_parent_dir(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create token dir {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}
