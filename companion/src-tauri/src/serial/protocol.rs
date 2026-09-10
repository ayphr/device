use serialport::SerialPort;
use std::collections::HashMap;
use std::sync::Mutex;

use super::constants::SERIAL_BAUD_RATE;
use crate::constants::COMMAND_TIMEOUT;
use crate::protocol::parse_status_response;
use crate::types::ParsedStatus;

static PORT_CACHE: Mutex<Option<HashMap<String, Box<dyn SerialPort>>>> = Mutex::new(None);

pub fn query_status(port_name: &str) -> Result<ParsedStatus, String> {
    let response = send_command(port_name, vec![ayphr_protocol::COMMAND_GET_STATUS])?;
    parse_status_response(&response)
}

pub fn send_command(port_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut cache = PORT_CACHE
        .lock()
        .map_err(|_| "Serial port cache mutex poisoned".to_string())?;
    let cache = cache.get_or_insert_with(HashMap::new);

    if !cache.contains_key(port_name) {
        let port = open_port(port_name)?;
        cache.insert(port_name.to_string(), port);
    }

    let port = cache
        .get_mut(port_name)
        .expect("port inserted above or already present");

    if let Err(error) = write_frame(&mut **port, &payload) {
        cache.remove(port_name);
        return Err(error);
    }

    match read_frame(&mut **port) {
        Ok(data) => Ok(data),
        Err(error) => {
            cache.remove(port_name);
            Err(error)
        }
    }
}

pub fn invalidate_port(port_name: &str) {
    if let Ok(mut cache) = PORT_CACHE.lock() {
        if let Some(cache) = cache.as_mut() {
            cache.remove(port_name);
        }
    }
}

fn open_port(port_name: &str) -> Result<Box<dyn SerialPort>, String> {
    serialport::new(port_name, SERIAL_BAUD_RATE)
        .timeout(COMMAND_TIMEOUT)
        .open()
        .map_err(|error| format!("Failed to open serial port {port_name}: {error}"))
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