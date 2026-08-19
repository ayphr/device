use serialport::SerialPort;
use tracing::{debug, warn};

use ayphr_protocol::{COMMAND_GET_STATUS, RESPONSE_STATUS};

use super::constants::COMMAND_TIMEOUT;
use crate::ble::state::ParsedStatus;

pub fn query_status(port_name: &str) -> Result<ParsedStatus, String> {
    debug!("[serial] querying device status on {}", port_name);
    let response = send_command(port_name, vec![COMMAND_GET_STATUS])?;
    debug!("[serial] status response bytes={}", format_bytes(&response));
    parse_status_response(&response)
}

pub fn parse_status_response(payload: &[u8]) -> Result<ParsedStatus, String> {
    if payload.len() < 7 || payload[0] != RESPONSE_STATUS {
        warn!("[serial] invalid status payload={}", format_bytes(payload));
        return Err("Invalid status response payload".to_string());
    }

    let setup_complete = payload[1] == 1;
    let authenticated = payload[2] == 1;
    let auth_required = payload[3] == 1;
    let wifi_required = payload[4] == 1;
    let name_length = payload[6] as usize;

    if payload.len() < 7 + name_length {
        warn!("[serial] status response missing device name bytes");
        return Err("Status response is missing device name bytes".to_string());
    }

    let device_name = String::from_utf8(payload[7..7 + name_length].to_vec())
        .map_err(|error| {
            warn!("[serial] device name decode failed: {}", error);
            "Device name is not valid UTF-8".to_string()
        })?;

    debug!(
        "[serial] parsed status setup_complete={} authenticated={} auth_required={} wifi_required={} device_name={}",
        setup_complete, authenticated, auth_required, wifi_required, device_name
    );

    Ok(ParsedStatus {
        setup_complete,
        authenticated,
        auth_required,
        wifi_required,
        device_name,
    })
}

pub fn send_command(port_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut port = open_port(port_name)?;
    write_frame(&mut *port, &payload)?;
    read_frame(&mut *port)
}

fn open_port(port_name: &str) -> Result<Box<dyn SerialPort>, String> {
    let mut port = serialport::new(port_name, super::constants::SERIAL_BAUD_RATE)
        .timeout(COMMAND_TIMEOUT)
        .open()
        .map_err(|error| format!("Failed to open serial port {port_name}: {error}"))?;

    let _ = port.write_data_terminal_ready(false);
    let _ = port.write_request_to_send(false);

    Ok(port)
}

fn write_frame(port: &mut dyn SerialPort, payload: &[u8]) -> Result<(), String> {
    if payload.len() > u16::MAX as usize {
        return Err("Payload too large for serial frame".to_string());
    }

    let len = payload.len() as u16;
    let mut frame = Vec::with_capacity(2 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(payload);
    port.write_all(&frame)
        .map_err(|error| format!("Failed to write serial frame: {error}"))?;
    port.flush()
        .map_err(|error| format!("Failed to flush serial frame: {error}"))
}

fn read_frame(port: &mut dyn SerialPort) -> Result<Vec<u8>, String> {
    let mut len_buf = [0u8; 2];
    port.read_exact(&mut len_buf)
        .map_err(|error| format!("Failed to read serial frame length: {error}"))?;
    let len = u16::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    port.read_exact(&mut payload)
        .map_err(|error| format!("Failed to read serial frame payload: {error}"))?;
    Ok(payload)
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
