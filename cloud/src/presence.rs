use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PresenceRecord {
    pub(crate) device_id: String,
    pub(crate) online: bool,
    pub(crate) last_seen_epoch_sec: u64,
}

#[derive(Debug, Default)]
pub(crate) struct PresenceTracker {
    last_seen_epoch_sec: HashMap<String, u64>,
    online: HashMap<String, bool>,
}

impl PresenceTracker {
    pub(crate) fn mark_online(&mut self, device_id: &str, now_sec: u64) -> bool {
        self.last_seen_epoch_sec
            .insert(device_id.to_string(), now_sec);
        let prev = self
            .online
            .insert(device_id.to_string(), true)
            .unwrap_or(false);
        !prev
    }

    pub(crate) fn scan_offline(&mut self, now_sec: u64, timeout_sec: u64) -> Vec<String> {
        let mut changed = Vec::new();
        for (device_id, last_seen) in &self.last_seen_epoch_sec {
            let is_online = now_sec.saturating_sub(*last_seen) <= timeout_sec;
            let prev = self.online.get(device_id).copied().unwrap_or(false);
            if prev && !is_online {
                self.online.insert(device_id.clone(), false);
                changed.push(device_id.clone());
            }
        }
        changed
    }

    pub(crate) fn snapshot(&self) -> Vec<PresenceRecord> {
        let mut out = Vec::new();
        for (device_id, last_seen) in &self.last_seen_epoch_sec {
            out.push(PresenceRecord {
                device_id: device_id.clone(),
                online: self.online.get(device_id).copied().unwrap_or(false),
                last_seen_epoch_sec: *last_seen,
            });
        }
        out.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        out
    }
}
