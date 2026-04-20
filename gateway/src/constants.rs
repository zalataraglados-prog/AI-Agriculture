#[allow(dead_code)]
pub const DEFAULT_TARGET: &str = "127.0.0.1:9000";
pub const DEFAULT_SCAN_INTERVAL_MS: u64 = 5000;
pub const DEFAULT_SCAN_WINDOW_MS: u64 = 1800;
pub const DEFAULT_ACK_TIMEOUT_MS: u64 = 3000;
pub const DEFAULT_DEVICE_LOOP_SLEEP_MS: u64 = 1000;
pub const DEFAULT_BAUD_LIST: [u32; 1] = [9600];
pub const DEFAULT_STATE_DIR: &str = "state";
pub const DEFAULT_IMAGE_UPLOAD_INTERVAL_MS: u64 = 300_000;
pub const DEFAULT_IMAGE_UPLOAD_PATH: &str = "/api/v1/image/upload";
pub const PROFILE_FILE: &str = "gateway_profile.json";
pub const DEVICE_INDEX_FILE: &str = "device_index.json";
#[allow(dead_code)]
pub const DEFAULT_PAYLOAD_SUCCESS: &str = "success";
pub const RESERVED_IMAGE_SENSOR_ID: &str = "image";
pub const RESERVED_IMAGE_FEATURE: &str = "image";
