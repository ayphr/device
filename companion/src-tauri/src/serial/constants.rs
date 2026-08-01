use std::time::Duration;

pub const SERIAL_DEVICES_UPDATED_EVENT: &str = "serial-devices-updated";
pub const DEVICE_RETENTION_WINDOW: Duration = Duration::from_secs(30);
pub const SCAN_INTERVAL: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
pub const SERIAL_BAUD_RATE: u32 = 115_200;
