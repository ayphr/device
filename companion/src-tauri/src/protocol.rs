use ayphr_protocol::{RESPONSE_FIRMWARE_INFO, RESPONSE_STATUS};

use crate::types::{ParsedFirmwareInfo, ParsedStatus};

pub fn parse_status_response(payload: &[u8]) -> Result<ParsedStatus, String> {
    if payload.len() < 7 || payload[0] != RESPONSE_STATUS {
        tracing::warn!("[protocol] invalid status payload={}", format_bytes(payload));
        return Err("Invalid status response payload".to_string());
    }

    let setup_complete = payload[1] == 1;
    let authenticated = payload[2] == 1;
    let auth_required = payload[3] == 1;
    let wifi_required = payload[4] == 1;
    let name_length = payload[6] as usize;

    if payload.len() < 7 + name_length {
        tracing::warn!("[protocol] status response missing device name bytes");
        return Err("Status response is missing device name bytes".to_string());
    }

    let device_name = String::from_utf8(payload[7..7 + name_length].to_vec())
        .map_err(|error| {
            tracing::warn!("[protocol] device name decode failed: {}", error);
            "Device name is not valid UTF-8".to_string()
        })?;

    Ok(ParsedStatus {
        setup_complete,
        authenticated,
        auth_required,
        wifi_required,
        device_name,
    })
}

pub fn parse_firmware_info_response(payload: &[u8]) -> Result<ParsedFirmwareInfo, String> {
    if payload.len() < 2 || payload[0] != RESPONSE_FIRMWARE_INFO {
        tracing::warn!("[protocol] invalid firmware info payload={}", format_bytes(payload));
        return Err("Invalid firmware info response payload".to_string());
    }

    let mut cursor = 1;

    let version = read_string_field(payload, &mut cursor)
        .map_err(|e| format!("Failed to read firmware version: {}", e))?;
    let hardware_rev = read_string_field(payload, &mut cursor)
        .map_err(|e| format!("Failed to read hardware revision: {}", e))?;

    if cursor + 4 > payload.len() {
        return Err("Firmware info response truncated before uptime".to_string());
    }
    let uptime_secs = u32::from_le_bytes([
        payload[cursor],
        payload[cursor + 1],
        payload[cursor + 2],
        payload[cursor + 3],
    ]);

    Ok(ParsedFirmwareInfo {
        version,
        hardware_rev,
        uptime_secs,
    })
}

fn read_string_field(payload: &[u8], cursor: &mut usize) -> Result<String, String> {
    if *cursor >= payload.len() {
        return Err("payload truncated at string field".to_string());
    }
    let len = payload[*cursor] as usize;
    *cursor += 1;
    if *cursor + len > payload.len() {
        return Err("payload truncated inside string field".to_string());
    }
    let val = String::from_utf8(payload[*cursor..*cursor + len].to_vec())
        .map_err(|e| format!("invalid UTF-8: {}", e))?;
    *cursor += len;
    Ok(val)
}

pub fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn log_string_error(context: &str, error: impl std::fmt::Display, prefix: &str) -> String {
    let message = error.to_string();
    tracing::error!("[{}] {}: {}", prefix, context, message);
    message
}
