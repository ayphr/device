use std::time::Duration;

pub const BLE_DEVICES_UPDATED_EVENT: &str = "ble-devices-updated";
pub const DEVICE_RETENTION_WINDOW: Duration = Duration::from_secs(30);
pub const SCAN_INTERVAL: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

pub const FIRMWARE_SERVICE_UUID: &str = "01171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_RX_CHARACTERISTIC_UUID: &str = "02171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_TX_CHARACTERISTIC_UUID: &str = "03171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_MANUFACTURER_ID: u16 = 0x4F4D;
