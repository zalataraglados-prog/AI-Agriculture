use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::{DEVICE_INDEX_FILE, FEATURE_MAP_FILE, PROFILE_FILE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayProfile {
    pub cloud_target: String,
    pub farm_location: String,
    pub crop_type: String,
    pub farm_note: String,
    pub last_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMapStore {
    pub mappings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIndexStore {
    pub port_to_device_id: BTreeMap<String, String>,
}

pub fn ensure_state_dir(state_dir: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(state_dir);
    if !path.exists() {
        fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create state dir {}: {e}", path.display()))?;
    }
    Ok(path)
}

pub fn load_profile(state_dir: &Path) -> Result<Option<GatewayProfile>, String> {
    let path = state_dir.join(PROFILE_FILE);
    read_json::<GatewayProfile>(&path)
}

pub fn save_profile(state_dir: &Path, profile: &GatewayProfile) -> Result<(), String> {
    let path = state_dir.join(PROFILE_FILE);
    write_json(&path, profile)
}

pub fn load_feature_map(state_dir: &Path) -> Result<FeatureMapStore, String> {
    let path = state_dir.join(FEATURE_MAP_FILE);
    match read_json::<FeatureMapStore>(&path)? {
        Some(mut store) => {
            inject_default_feature_mappings(&mut store.mappings);
            Ok(store)
        }
        None => {
            let mut mappings = BTreeMap::new();
            inject_default_feature_mappings(&mut mappings);
            Ok(FeatureMapStore { mappings })
        }
    }
}

pub fn save_feature_map(state_dir: &Path, store: &FeatureMapStore) -> Result<(), String> {
    let path = state_dir.join(FEATURE_MAP_FILE);
    write_json(&path, store)
}

pub fn load_device_index(state_dir: &Path) -> Result<DeviceIndexStore, String> {
    let path = state_dir.join(DEVICE_INDEX_FILE);
    match read_json::<DeviceIndexStore>(&path)? {
        Some(store) => Ok(store),
        None => Ok(DeviceIndexStore {
            port_to_device_id: BTreeMap::new(),
        }),
    }
}

pub fn save_device_index(state_dir: &Path, store: &DeviceIndexStore) -> Result<(), String> {
    let path = state_dir.join(DEVICE_INDEX_FILE);
    write_json(&path, store)
}

pub fn reset_state(state_dir: &str) -> Result<(), String> {
    let state_path = PathBuf::from(state_dir);
    if !state_path.exists() {
        return Ok(());
    }

    for file in [PROFILE_FILE, FEATURE_MAP_FILE, DEVICE_INDEX_FILE] {
        let path = state_path.join(file);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        }
    }

    Ok(())
}

fn inject_default_feature_mappings(mappings: &mut BTreeMap<String, String>) {
    for (feature, sensor) in [
        ("mq7", "mq7"),
        ("dht22", "dht22"),
        ("adc", "adc"),
        ("pcf8591", "pcf8591"),
    ] {
        mappings
            .entry(feature.to_string())
            .or_insert_with(|| sensor.to_string());
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let value = serde_json::from_str::<T>(&content)
        .map_err(|e| format!("Failed to parse JSON {}: {e}", path.display()))?;
    Ok(Some(value))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize {}: {e}", path.display()))?;
    fs::write(path, content).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

