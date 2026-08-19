use esp_idf_svc::hal::delay::BLOCK;
use esp_idf_svc::hal::uart::UartDriver;
use log::{info, warn};
use std::time::Duration;

use crate::command_processor;
use crate::config::DeviceSetup;

const MAX_PAYLOAD_LEN: usize = 4096;

pub fn run_loop(uart: &UartDriver<'_>, setup: &DeviceSetup) {
    loop {
        let mut length_buf = [0u8; 2];
        if let Err(error) = read_exact(uart, &mut length_buf) {
            warn!("Serial frame length read failed: {:?}", error);
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        let payload_len = u16::from_le_bytes(length_buf) as usize;
        if payload_len == 0 {
            continue;
        }

        if payload_len > MAX_PAYLOAD_LEN {
            warn!(
                "Serial frame payload too large: {} bytes (max {})",
                payload_len, MAX_PAYLOAD_LEN
            );
            continue;
        }

        let mut payload = vec![0u8; payload_len];
        if let Err(error) = read_exact(uart, &mut payload) {
            warn!("Serial frame payload read failed: {:?}", error);
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        info!("Received serial request bytes={}", payload.len());
        let response = command_processor::process_serial_request(setup, &payload);

        if let Err(error) = write_frame(uart, &response) {
            warn!("Failed to write serial response: {:?}", error);
        }
    }
}

fn read_exact(uart: &UartDriver<'_>, mut buf: &mut [u8]) -> anyhow::Result<()> {
    while !buf.is_empty() {
        let read = uart.read(buf, BLOCK)?;
        if read == 0 {
            continue;
        }
        let tmp = buf;
        buf = &mut tmp[read..];
    }
    Ok(())
}

fn write_frame(uart: &UartDriver<'_>, payload: &[u8]) -> anyhow::Result<()> {
    let len = u16::try_from(payload.len())?;
    let mut frame = Vec::with_capacity(2 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(payload);

    let mut written = 0;
    while written < frame.len() {
        written += uart.write(&frame[written..])?;
    }

    Ok(())
}
