pub const COMMAND_GET_STATUS: u8 = 0x01;
pub const COMMAND_AUTHENTICATE: u8 = 0x02;
pub const COMMAND_APPLY_SETUP: u8 = 0x03;
pub const COMMAND_RESTART: u8 = 0x04;
pub const COMMAND_FACTORY_RESET: u8 = 0x05;
pub const COMMAND_CHANGE_PASSWORD: u8 = 0x06;
pub const COMMAND_UPDATE_WIFI: u8 = 0x07;
pub const COMMAND_GET_FIRMWARE_INFO: u8 = 0x08;
pub const COMMAND_OTA_BEGIN: u8 = 0x09;
pub const COMMAND_OTA_DATA: u8 = 0x0A;
pub const COMMAND_OTA_END: u8 = 0x0B;
pub const COMMAND_OTA_ROLLBACK: u8 = 0x0C;
pub const COMMAND_RENAME_DEVICE: u8 = 0x0D;
pub const COMMAND_GET_LIVE_DATA: u8 = 0x0E;
pub const COMMAND_GET_DEVICE_SETTINGS: u8 = 0x0F;
pub const COMMAND_UPDATE_DEVICE_SETTINGS: u8 = 0x10;
pub const COMMAND_CLOUD_LINK_START: u8 = 0x11;
pub const COMMAND_CLOUD_LINK_STATUS: u8 = 0x12;
pub const COMMAND_CLOUD_UNLINK: u8 = 0x13;
pub const COMMAND_GET_DEVICE_IDENTITY: u8 = 0x14;
pub const COMMAND_STORE_CLOUD_TOKEN: u8 = 0x15;

pub const RESPONSE_STATUS: u8 = 0x81;
pub const RESPONSE_AUTH_OK: u8 = 0x82;
pub const RESPONSE_AUTH_FAILED: u8 = 0x83;
pub const RESPONSE_SETUP_OK: u8 = 0x84;
pub const RESPONSE_RESTART_OK: u8 = 0x85;
pub const RESPONSE_FACTORY_RESET_OK: u8 = 0x86;
pub const RESPONSE_CHANGE_PASSWORD_OK: u8 = 0x87;
pub const RESPONSE_SETUP_FAILED: u8 = 0x88;
pub const RESPONSE_UPDATE_WIFI_OK: u8 = 0x89;
pub const RESPONSE_FIRMWARE_INFO: u8 = 0x8A;
pub const RESPONSE_OTA_BEGIN_OK: u8 = 0x8B;
pub const RESPONSE_OTA_DATA_OK: u8 = 0x8C;
pub const RESPONSE_OTA_END_OK: u8 = 0x8D;
pub const RESPONSE_OTA_ROLLBACK_OK: u8 = 0x8E;
pub const RESPONSE_RENAME_OK: u8 = 0x8F;
pub const RESPONSE_AUTH_LOCKED: u8 = 0x90;
pub const RESPONSE_LIVE_DATA: u8 = 0x91;
pub const RESPONSE_DEVICE_SETTINGS: u8 = 0x92;
pub const RESPONSE_DEVICE_SETTINGS_OK: u8 = 0x93;
pub const RESPONSE_CLOUD_LINK: u8 = 0x94;
pub const RESPONSE_CLOUD_UNLINK_OK: u8 = 0x95;
pub const RESPONSE_DEVICE_IDENTITY: u8 = 0x96;
pub const RESPONSE_ERROR: u8 = 0xFF;

pub const SERIAL_CHUNK_SIZE: usize = 4096;
pub const BLE_CHUNK_SIZE: usize = 1024;

pub const BLE_FRAME_HEADER_LEN: usize = 2;
pub const BLE_NOTIFY_CHUNK_SIZE: usize = 20;
pub const MAX_BLE_FRAME_PAYLOAD: usize = 4096;

pub const MAX_DEVICE_NAME_LEN: usize = 64;

pub const MIN_SAMPLING_INTERVAL_SECS: u32 = 1;
pub const MAX_SAMPLING_INTERVAL_SECS: u32 = 6 * 60 * 60;
pub const DEFAULT_SAMPLING_INTERVAL_SECS: u32 = 5;

pub const MIN_UPLOAD_INTERVAL_SECS: u32 = 60;
pub const MAX_UPLOAD_INTERVAL_SECS: u32 = 6 * 60 * 60;
pub const DEFAULT_UPLOAD_INTERVAL_SECS: u32 = 60;

/// Maximum number of points the cloud API accepts in a single upload.
pub const MAX_TELEMETRY_POINTS_PER_UPLOAD: usize = 2000;
/// Bounded on-device buffer. Oldest points are dropped once it is full.
pub const MAX_TELEMETRY_BUFFER_POINTS: usize = 4096;

pub const CLOUD_STATE_UNLINKED: u8 = 0;
pub const CLOUD_STATE_REQUESTING: u8 = 1;
pub const CLOUD_STATE_PENDING: u8 = 2;
pub const CLOUD_STATE_LINKED: u8 = 3;
pub const CLOUD_STATE_ERROR: u8 = 4;

pub fn clamp_sampling_interval(secs: u32) -> u32 {
    secs.clamp(MIN_SAMPLING_INTERVAL_SECS, MAX_SAMPLING_INTERVAL_SECS)
}

pub fn clamp_upload_interval(secs: u32, sampling_interval_secs: u32) -> u32 {
    secs.max(MIN_UPLOAD_INTERVAL_SECS)
        .max(clamp_sampling_interval(sampling_interval_secs))
        .min(MAX_UPLOAD_INTERVAL_SECS)
}

/// Returns true when both intervals are in range and the upload interval is
/// at least as long as the sampling interval.
pub fn intervals_valid(sampling_interval_secs: u32, upload_interval_secs: u32) -> bool {
    (MIN_SAMPLING_INTERVAL_SECS..=MAX_SAMPLING_INTERVAL_SECS).contains(&sampling_interval_secs)
        && (MIN_UPLOAD_INTERVAL_SECS..=MAX_UPLOAD_INTERVAL_SECS).contains(&upload_interval_secs)
        && upload_interval_secs >= sampling_interval_secs
}

pub const FIRMWARE_SERVICE_UUID: &str = "01171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_RX_CHARACTERISTIC_UUID: &str = "02171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_TX_CHARACTERISTIC_UUID: &str = "03171718-ce62-6a9a-5541-b839b04a7bd1";
pub const FIRMWARE_MANUFACTURER_ID: u16 = 0x4F4D;
