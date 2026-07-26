use std::time::Duration;

pub const BLE_DEVICES_UPDATED_EVENT: &str = "ble-devices-updated";
pub const DEVICE_RETENTION_WINDOW: Duration = Duration::from_secs(30);
pub const SCAN_INTERVAL: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
