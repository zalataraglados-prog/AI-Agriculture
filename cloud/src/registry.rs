use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{DeviceRegistryFile, RegisterOutcome, RegisterRequest, RegisteredDevice};

pub(crate) struct DeviceRegistry {
    path: String,
    inner: DeviceRegistryFile,
}

impl DeviceRegistry {
    pub(crate) fn load(path: &str) -> Result<Self, String> {
        ensure_parent_dir(path)?;

        if !Path::new(path).exists() {
            return Ok(Self {
                path: path.to_string(),
                inner: DeviceRegistryFile::default(),
            });
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read registry {}: {e}", path))?;
        let inner = serde_json::from_str::<DeviceRegistryFile>(&content)
            .map_err(|e| format!("Failed to parse registry {}: {e}", path))?;

        Ok(Self {
            path: path.to_string(),
            inner,
        })
    }

    pub(crate) fn register_device(&mut self, request: RegisterRequest) -> Result<RegisterOutcome, String> {
        if request.device_id.trim().is_empty() {
            return Err("register request missing device_id".to_string());
        }

        let incoming_fp = fingerprint(&request.location, &request.crop_type, &request.sensors);

        for (device_id, existing) in &self.inner.devices {
            if *device_id == request.device_id {
                continue;
            }
            let existing_fp = fingerprint(&existing.location, &existing.crop_type, &existing.sensors);
            if existing_fp == incoming_fp {
                return Ok(RegisterOutcome::Conflict);
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let device = RegisteredDevice {
            device_id: request.device_id.clone(),
            location: request.location,
            crop_type: request.crop_type,
            farm_note: request.farm_note,
            sensors: request.sensors,
            feature_mapping: request.feature_mapping,
            registered_at_epoch_sec: now,
        };

        self.inner.devices.insert(request.device_id, device);
        self.save()?;

        Ok(RegisterOutcome::Ok)
    }

    pub(crate) fn is_registered(&self, device_id: &str) -> bool {
        self.inner.devices.contains_key(device_id)
    }
}

fn fingerprint(location: &str, crop_type: &str, sensors: &[String]) -> String {
    let mut unique = HashSet::new();
    let mut list = Vec::new();
    for sensor in sensors {
        let trimmed = sensor.trim();
        if trimmed.is_empty() {
            continue;
        }
        if unique.insert(trimmed.to_string()) {
            list.push(trimmed.to_string());
        }
    }
    list.sort();
    format!(
        "{}|{}|{}",
        location.trim().to_ascii_lowercase(),
        crop_type.trim().to_ascii_lowercase(),
        list.join(",")
    )
}

impl DeviceRegistry {
    fn save(&self) -> Result<(), String> {
        let content = serde_json::to_string_pretty(&self.inner)
            .map_err(|e| format!("Failed to serialize registry: {e}"))?;
        fs::write(&self.path, content)
            .map_err(|e| format!("Failed to write registry {}: {e}", self.path))
    }
}

fn ensure_parent_dir(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create registry dir {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}

