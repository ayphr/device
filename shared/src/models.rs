use crate::constants::{RESPONSE_FIRMWARE_INFO, RESPONSE_STATUS};
use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceStatusResponse {
    pub is_configured: bool,
    pub is_authenticated: bool,
    pub auth_required: bool,
    pub wifi_required: bool,
    pub has_wifi: bool,
    pub device_name: String,
}

impl DeviceStatusResponse {
    pub fn parse(payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() < 6 || payload[0] != RESPONSE_STATUS {
            return Err("Invalid status payload or opcode");
        }

        let is_configured = payload[1] == 1;
        let is_authenticated = payload[2] == 1;
        let auth_required = payload[3] == 1;
        let wifi_required = payload[4] == 1;
        let has_wifi = payload[5] == 1;
        let name_len = payload[6] as usize;

        if payload.len() < 7 + name_len {
            return Err("Status payload truncated before reading device name");
        }

        let device_name = core::str::from_utf8(&payload[7..7 + name_len])
            .map_err(|_| "Device name contains invalid UTF-8")?
            .into();

        Ok(Self {
            is_configured,
            is_authenticated,
            auth_required,
            wifi_required,
            has_wifi,
            device_name,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SensorTelemetry {
    pub temperature_c: f32,
    pub humidity_pct: f32,
    pub pressure_hpa: f32,
}

impl SensorTelemetry {
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&self.temperature_c.to_le_bytes());
        buf[4..8].copy_from_slice(&self.humidity_pct.to_le_bytes());
        buf[8..12].copy_from_slice(&self.pressure_hpa.to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 12 {
            return Err("Telemetry packet buffer underflow");
        }
        Ok(Self {
            temperature_c: f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            humidity_pct: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            pressure_hpa: f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareInfoResponse {
    pub version: String,
    pub hardware_rev: String,
    pub uptime_secs: u32,
}

impl FirmwareInfoResponse {
    pub fn parse(payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() < 2 || payload[0] != RESPONSE_FIRMWARE_INFO {
            return Err("Invalid firmware info payload or opcode");
        }

        let mut cursor = 1;

        let version = Self::read_string_field(payload, &mut cursor)?;
        let hardware_rev = Self::read_string_field(payload, &mut cursor)?;

        if cursor + 4 > payload.len() {
            return Err("Firmware info payload truncated before uptime");
        }
        let uptime_secs = u32::from_le_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]);

        Ok(Self {
            version,
            hardware_rev,
            uptime_secs,
        })
    }

    fn read_string_field(payload: &[u8], cursor: &mut usize) -> Result<String, &'static str> {
        if *cursor >= payload.len() {
            return Err("Firmware info payload truncated at string field");
        }
        let len = payload[*cursor] as usize;
        *cursor += 1;
        if *cursor + len > payload.len() {
            return Err("Firmware info payload truncated inside string field");
        }
        let val = core::str::from_utf8(&payload[*cursor..*cursor + len])
            .map_err(|_| "Firmware info contains invalid UTF-8")?
            .into();
        *cursor += len;
        Ok(val)
    }
}
