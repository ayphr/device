use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleConnectionState {
    pub connected: bool,
    pub authenticated: bool,
    pub auth_required: bool,
    pub wifi_required: bool,
    pub setup_complete: bool,
    pub device_name: String,
}

#[derive(Clone)]
pub struct ParsedStatus {
    pub setup_complete: bool,
    pub authenticated: bool,
    pub auth_required: bool,
    pub wifi_required: bool,
    pub device_name: String,
}

#[derive(Clone)]
pub struct ParsedFirmwareInfo {
    pub version: String,
    pub hardware_rev: String,
    pub uptime_secs: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareInfoResult {
    pub version: String,
    pub hardware_rev: String,
    pub uptime_secs: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareUpdateProgress {
    pub step: String,
    pub progress: f32,
    pub message: String,
}

#[derive(Clone, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareReleaseMetadata {
    pub version: Option<String>,
    pub binary_url: Option<String>,
    pub sha256: Option<String>,
}
