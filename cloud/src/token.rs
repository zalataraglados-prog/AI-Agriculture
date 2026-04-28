use std::collections::HashMap;

use std::fs;

use std::path::Path;

use std::time::{SystemTime, UNIX_EPOCH};



use rand::distributions::Alphanumeric;

use rand::{thread_rng, Rng};

use serde::{Deserialize, Serialize};



#[derive(Debug, Default, Serialize, Deserialize)]

struct TokenStore {

    hourly_tokens: HashMap<String, String>,

}



pub(crate) fn current_hour_token(path: &str) -> Result<String, String> {

    let mut store = load_store(path)?;

    let key = current_hour_key();

    if let Some(token) = store.hourly_tokens.get(&key) {

        return Ok(token.clone());

    }



    let token = generate_token();

    store.hourly_tokens.insert(key.clone(), token.clone());

    prune_old_hours(&mut store.hourly_tokens, &key);

    save_store(path, &store)?;



    Ok(token)

}



pub(crate) fn validate_current_hour_token(path: &str, candidate: &str) -> Result<bool, String> {

    let expected = current_hour_token(path)?;

    Ok(expected == candidate)

}



fn load_store(path: &str) -> Result<TokenStore, String> {

    ensure_parent_dir(path)?;

    if !Path::new(path).exists() {

        return Ok(TokenStore::default());

    }



    let content = fs::read_to_string(path)

        .map_err(|e| format!("Failed to read token store {}: {e}", path))?;

    let store = serde_json::from_str::<TokenStore>(&content)

        .map_err(|e| format!("Failed to parse token store {}: {e}", path))?;

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
