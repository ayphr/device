use ayphr_protocol::{
    unpack, SensorPacket, RESPONSE_FIRMWARE_INFO, RESPONSE_LIVE_DATA, RESPONSE_STATUS,
};

use crate::types::{ParsedFirmwareInfo, ParsedStatus};

pub fn parse_status_response(payload: &[u8]) -> Result<ParsedStatus, String> {
    if payload.len() < 7 || payload[0] != RESPONSE_STATUS {
        tracing::warn!(
            "[protocol] invalid status payload={}",
            format_bytes(payload)
        );
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

    let device_name = String::from_utf8(payload[7..7 + name_length].to_vec()).map_err(|error| {
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
        tracing::warn!(
            "[protocol] invalid firmware info payload={}",
            format_bytes(payload)
        );
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

pub fn parse_live_data_response(payload: &[u8]) -> Result<Option<SensorPacket>, String> {
    if payload.len() < 2 || payload[0] != RESPONSE_LIVE_DATA {
        tracing::warn!(
            "[protocol] invalid live data payload={}",
            format_bytes(payload)
        );
        return Err("Invalid live data response payload".to_string());
    }

    if payload[1] == 0 {
        return Ok(None);
    }

    let decoded = unpack(&payload[2..]).ok_or_else(|| {
        tracing::warn!(
            "[protocol] malformed live data packet={}",
            format_bytes(payload)
        );
        "Live data packet is malformed".to_string()
    })?;

    if !decoded.valid {
        tracing::warn!("[protocol] live data packet failed parity check");
        return Err("Live data packet failed its parity check".to_string());
    }

    Ok(Some(decoded.packet))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(&[0x01, 0x0A, 0xFF]), "01 0a ff");
        assert_eq!(format_bytes(&[]), "");
    }

    #[test]
    fn test_parse_status_response_valid() {
        let name = "My Device";
        let mut payload = vec![
            RESPONSE_STATUS,
            1, // setup_complete
            1, // authenticated
            1, // auth_required
            1, // wifi_required
            1, // wifi_configured
            name.len() as u8,
        ];
        payload.extend_from_slice(name.as_bytes());

        let parsed = parse_status_response(&payload).unwrap();
        assert!(parsed.setup_complete);
        assert!(parsed.authenticated);
        assert!(parsed.auth_required);
        assert!(parsed.wifi_required);
        assert_eq!(parsed.device_name, "My Device");
    }

    #[test]
    fn test_parse_status_response_invalid_tag() {
        let payload = vec![0x99, 1, 1, 1, 1, 1, 0];
        assert!(parse_status_response(&payload).is_err());
    }

    #[test]
    fn test_parse_status_response_truncated() {
        let payload = vec![RESPONSE_STATUS, 1, 1, 1, 1, 1, 10, b'a', b'b'];
        assert!(parse_status_response(&payload).is_err());
    }

    #[test]
    fn test_parse_firmware_info_response_valid() {
        let version = "0.1.0";
        let hw_rev = "rev1";
        let uptime: u32 = 3600;

        let mut payload = vec![RESPONSE_FIRMWARE_INFO];
        payload.push(version.len() as u8);
        payload.extend_from_slice(version.as_bytes());
        payload.push(hw_rev.len() as u8);
        payload.extend_from_slice(hw_rev.as_bytes());
        payload.extend_from_slice(&uptime.to_le_bytes());

        let parsed = parse_firmware_info_response(&payload).unwrap();
        assert_eq!(parsed.version, "0.1.0");
        assert_eq!(parsed.hardware_rev, "rev1");
        assert_eq!(parsed.uptime_secs, 3600);
    }

    #[test]
    fn test_parse_live_data_response_valid() {
        let packet = SensorPacket {
            packet_type: ayphr_protocol::PACKET_TYPE_SENSOR,
            temperature: 21.5,
            humidity: 48.0,
            air_pressure: 1013,
            serial: 42,
        };
        let encoded = ayphr_protocol::pack(&packet).unwrap();

        let mut payload = vec![RESPONSE_LIVE_DATA, 1];
        payload.extend_from_slice(&encoded);

        let parsed = parse_live_data_response(&payload).unwrap().unwrap();
        assert_eq!(parsed, packet);
    }

    #[test]
    fn test_parse_live_data_response_unavailable() {
        let payload = vec![RESPONSE_LIVE_DATA, 0];
        assert_eq!(parse_live_data_response(&payload).unwrap(), None);
    }

    #[test]
    fn test_parse_live_data_response_rejects_bad_payloads() {
        assert!(parse_live_data_response(&[0x00, 1]).is_err());
        assert!(parse_live_data_response(&[RESPONSE_LIVE_DATA, 1, 0x02]).is_err());
    }
}
