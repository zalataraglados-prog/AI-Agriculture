use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};

use crate::model::{DeviceRegistryFile, RegisterOutcome, RegisterRequest, RegisteredDevice};

pub(crate) struct DeviceRegistry {
    path: String,
    inner: DeviceRegistryFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredentialValidation {
    Valid,
    Invalid,
    Revoked,
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

    pub(crate) fn register_device_with_token(
        &mut self,
        request: RegisterRequest,
        allowed_sensor_ids: &HashSet<String>,
    ) -> Result<(RegisterOutcome, Option<String>), String> {
        if request.device_id.trim().is_empty() {
            return Err("register request missing device_id".to_string());
        }

        let (normalized_sensors, normalized_mapping) =
            normalize_and_validate_contract(&request, allowed_sensor_ids)?;

        if self.has_conflict(
            &request.device_id,
            &request.location,
            &request.crop_type,
            &normalized_sensors,
        ) {
            return Ok((RegisterOutcome::Conflict, None));
        }

        let now = now_epoch_sec();
        let device_key = generate_device_key();
        let device = RegisteredDevice {
            device_id: request.device_id.clone(),
            location: request.location,
            crop_type: request.crop_type,
            farm_note: request.farm_note,
            sensors: normalized_sensors,
            feature_mapping: normalized_mapping,
            registered_at_epoch_sec: now,
            device_key_hash: hash_device_key(&device_key),
            credential_revoked: false,
            credential_issued_at_epoch_sec: now,
        };

        self.inner.devices.insert(request.device_id, device);
        self.save()?;

        Ok((RegisterOutcome::Ok, Some(device_key)))
    }

    pub(crate) fn register_device_with_credential(
        &mut self,
        request: RegisterRequest,
        allowed_sensor_ids: &HashSet<String>,
    ) -> Result<RegisterOutcome, String> {
        if request.device_id.trim().is_empty() {
            return Err("register request missing device_id".to_string());
        }

        let (normalized_sensors, normalized_mapping) =
            normalize_and_validate_contract(&request, allowed_sensor_ids)?;

        if self.has_conflict(
            &request.device_id,
            &request.location,
            &request.crop_type,
            &normalized_sensors,
        ) {
            return Ok(RegisterOutcome::Conflict);
        }

        let Some(device) = self.inner.devices.get_mut(&request.device_id) else {
            return Err("register request references unknown device_id".to_string());
        };

        device.location = request.location;
        device.crop_type = request.crop_type;
        device.farm_note = request.farm_note;
        device.sensors = normalized_sensors;
        device.feature_mapping = normalized_mapping;

        self.save()?;
        Ok(RegisterOutcome::Ok)
    }

    pub(crate) fn validate_device_credential(
        &self,
        device_id: &str,
        device_key: &str,
    ) -> CredentialValidation {
        let Some(device) = self.inner.devices.get(device_id) else {
            return CredentialValidation::Invalid;
        };

        if device.credential_revoked {
            return CredentialValidation::Revoked;
        }

        if device.device_key_hash.is_empty() {
            return CredentialValidation::Invalid;
        }

        if device.device_key_hash == hash_device_key(device_key) {
            CredentialValidation::Valid
        } else {
            CredentialValidation::Invalid
        }
    }

    pub(crate) fn is_registered(&self, device_id: &str) -> bool {
        self.inner.devices.contains_key(device_id)
    }

    pub(crate) fn is_sensor_allowed_for_device(&self, device_id: &str, sensor_id: &str) -> bool {
        let Some(device) = self.inner.devices.get(device_id) else {
            return false;
        };

        if device.sensors.is_empty() && device.feature_mapping.is_empty() {
            return true;
        }

        if device.sensors.iter().any(|v| v == sensor_id) {
            return true;
        }

        device.feature_mapping.values().any(|v| v == sensor_id)
    }

    fn has_conflict(
        &self,
        device_id: &str,
        location: &str,
        crop_type: &str,
        sensors: &[String],
    ) -> bool {
        let incoming_fp = fingerprint(location, crop_type, sensors);
        for (existing_id, existing) in &self.inner.devices {
            if existing_id == device_id {
                continue;
            }
            let existing_fp =
                fingerprint(&existing.location, &existing.crop_type, &existing.sensors);
            if existing_fp == incoming_fp {
                return true;
            }
        }
        false
    }
}

fn normalize_and_validate_contract(
    request: &RegisterRequest,
    allowed_sensor_ids: &HashSet<String>,
) -> Result<(Vec<String>, HashMap<String, String>), String> {
    let mut normalized_sensors = dedup_sensors(&request.sensors);
    normalized_sensors.sort();

    for sensor_id in &normalized_sensors {
        if !allowed_sensor_ids.contains(sensor_id) {
            return Err(format!(
                "register request contains unsupported sensor id: {sensor_id}"
            ));
        }
    }

    let mut normalized_mapping = HashMap::new();
    for (feature, sensor_id) in &request.feature_mapping {
        let feature = feature.trim();
        let sensor_id = sensor_id.trim();
        if feature.is_empty() || sensor_id.is_empty() {
            return Err("register request has empty feature_mapping entry".to_string());
        }
        if !allowed_sensor_ids.contains(sensor_id) {
            return Err(format!(
                "register request feature_mapping points to unsupported sensor id: {sensor_id}"
            ));
        }
        if !normalized_sensors.is_empty() && !normalized_sensors.iter().any(|v| v == sensor_id) {
            return Err(format!(
                "register request feature_mapping sensor not in sensors list: {sensor_id}"
            ));
        }
        normalized_mapping.insert(feature.to_string(), sensor_id.to_string());
    }

    Ok((normalized_sensors, normalized_mapping))
}

fn fingerprint(location: &str, crop_type: &str, sensors: &[String]) -> String {
    let mut list = dedup_sensors(sensors);
    list.sort();
    format!(
        "{}|{}|{}",
        location.trim().to_ascii_lowercase(),
        crop_type.trim().to_ascii_lowercase(),
        list.join(",")
    )
}

fn dedup_sensors(sensors: &[String]) -> Vec<String> {
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
    list
}

fn hash_device_key(device_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(device_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_device_key() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn now_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{CredentialValidation, DeviceRegistry};
    use crate::model::RegisterRequest;

    fn temp_registry_path() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("state/registry.test.{now}.json")
    }

    fn sample_request() -> RegisterRequest {
        RegisterRequest {
            device_id: "dev_modbus_01".to_string(),
            location: "field-a".to_string(),
            crop_type: "rice".to_string(),
            farm_note: String::new(),
            sensors: vec!["soil_modbus_02".to_string()],
            feature_mapping: HashMap::from([(
                "soil_modbus_02".to_string(),
                "soil_modbus_02".to_string(),
            )]),
            token: Some("dummy".to_string()),
            device_key: None,
        }
    }

    #[test]
    fn register_device_rejects_unknown_sensor() {
        let path = temp_registry_path();
        let mut registry = DeviceRegistry::load(&path).expect("load registry");
        let allowed = HashSet::from(["mq7".to_string()]);
        let err = registry
            .register_device_with_token(sample_request(), &allowed)
            .expect_err("register should fail");
        assert!(err.contains("unsupported sensor id"));
    }

    #[test]
    fn sensor_binding_checks_device_registration() {
        let path = temp_registry_path();
        let mut registry = DeviceRegistry::load(&path).expect("load registry");
        let allowed = HashSet::from(["soil_modbus_02".to_string()]);
        registry
            .register_device_with_token(sample_request(), &allowed)
            .expect("register should succeed");

        assert!(registry.is_sensor_allowed_for_device("dev_modbus_01", "soil_modbus_02"));
        assert!(!registry.is_sensor_allowed_for_device("dev_modbus_01", "mq7"));
    }

    #[test]
    fn device_credential_allows_re_registration_without_token() {
        let path = temp_registry_path();
        let mut registry = DeviceRegistry::load(&path).expect("load registry");
        let allowed = HashSet::from(["soil_modbus_02".to_string()]);

        let (_, issued_key) = registry
            .register_device_with_token(sample_request(), &allowed)
            .expect("register should succeed");
        let issued_key = issued_key.expect("issued key");

        assert_eq!(
            registry.validate_device_credential("dev_modbus_01", &issued_key),
            CredentialValidation::Valid
        );
        assert_eq!(
            registry.validate_device_credential("dev_modbus_01", "wrong-key"),
            CredentialValidation::Invalid
        );
    }
}
