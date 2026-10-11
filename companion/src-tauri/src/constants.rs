use std::time::Duration;

pub const DEVICE_RETENTION_WINDOW: Duration = Duration::from_secs(30);
pub const SCAN_INTERVAL: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
pub const BLE_OTA_WINDOW: usize = 16;
