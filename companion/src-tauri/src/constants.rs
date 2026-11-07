use std::time::Duration;

pub const DEVICE_RETENTION_WINDOW: Duration = Duration::from_secs(30);
pub const SCAN_INTERVAL: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
pub const OTA_BEGIN_TIMEOUT: Duration = Duration::from_secs(60);
pub const OTA_DATA_TIMEOUT: Duration = Duration::from_secs(60);
// The firmware processes each BLE request synchronously and sends its response
// before accepting the next request. Keep OTA writes serialized to avoid
// overflowing the peripheral's write/notification queue.
pub const BLE_OTA_WINDOW: usize = 1;
