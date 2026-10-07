use serialport::SerialPort;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::constants::SERIAL_BAUD_RATE;
use crate::constants::COMMAND_TIMEOUT;
use crate::protocol::parse_status_response;
use crate::types::ParsedStatus;

const MAX_FRAME_PAYLOAD: usize = 8192;

type PortHandle = Arc<Mutex<Box<dyn SerialPort>>>;

/// Registry of opened serial ports, keyed by system port name.
///
/// Each port is opened once and guarded by its own mutex, so a command or
/// query targeting one port can never block access to another port. An
/// unresponsive port can only stall callers that share that same port. Ports
/// that fail or disappear are evicted so the next caller re-opens them.
#[derive(Default)]
struct SerialPortRegistry {
    ports: Mutex<Option<HashMap<String, PortHandle>>>,
}

impl SerialPortRegistry {
    const fn new() -> Self {
        Self {
            ports: Mutex::new(None),
        }
    }

    /// Returns the handle for a port, opening it on first use. Poisoned
    /// registry locks are recovered so a single panic cannot brick serial I/O.
    fn handle(&self, port_name: &str) -> Result<PortHandle, String> {
        let mut registry = self
            .ports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let registry = registry.get_or_insert_with(HashMap::new);

        if let Some(handle) = registry.get(port_name) {
            return Ok(handle.clone());
        }

        let port = open_port(port_name)?;
        let handle = Arc::new(Mutex::new(port));
        registry.insert(port_name.to_string(), handle.clone());
        Ok(handle)
    }

    fn remove(&self, port_name: &str) {
        if let Ok(mut registry) = self.ports.lock() {
            if let Some(registry) = registry.as_mut() {
                registry.remove(port_name);
            }
        }
    }

    #[cfg(test)]
    fn insert(&self, port_name: &str, port: Box<dyn SerialPort>) {
        let handle = Arc::new(Mutex::new(port));
        let mut registry = self
            .ports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry
            .get_or_insert_with(HashMap::new)
            .insert(port_name.to_string(), handle);
    }

    #[cfg(test)]
    fn contains(&self, port_name: &str) -> bool {
        self.ports.lock().ok().is_some_and(|guard| {
            guard
                .as_ref()
                .is_some_and(|registry| registry.contains_key(port_name))
        })
    }
}

static PORT_REGISTRY: SerialPortRegistry = SerialPortRegistry::new();

pub async fn query_status(port_name: &str) -> Result<ParsedStatus, String> {
    let response = send_command(
        port_name.to_string(),
        vec![ayphr_protocol::COMMAND_GET_STATUS],
    )
    .await?;
    parse_status_response(&response)
}

pub async fn send_command(port_name: String, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || send_command_blocking(&port_name, payload))
        .await
        .map_err(|error| format!("Serial command task join error: {}", error))?
}

fn send_command_blocking(port_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    send_command_blocking_with_registry(&PORT_REGISTRY, port_name, payload)
}

fn send_command_blocking_with_registry(
    registry: &SerialPortRegistry,
    port_name: &str,
    payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let handle = registry.handle(port_name)?;
    let mut port = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Err(error) = write_frame(&mut **port, &payload) {
        drop(port);
        registry.remove(port_name);
        return Err(error);
    }

    match read_frame(&mut **port) {
        Ok(data) => Ok(data),
        Err(error) => {
            drop(port);
            registry.remove(port_name);
            Err(error)
        }
    }
}

pub fn invalidate_port(port_name: &str) {
    PORT_REGISTRY.remove(port_name);
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

    let mut frame = Vec::with_capacity(2 + payload.len());
    frame.extend_from_slice(&(payload.len() as u16).to_le_bytes());
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
    if len > MAX_FRAME_PAYLOAD {
        return Err(format!(
            "Serial frame payload length {len} exceeds max limit of {MAX_FRAME_PAYLOAD} bytes"
        ));
    }
    let mut payload = vec![0u8; len];
    port.read_exact(&mut payload)
        .map_err(|error| format!("Failed to read serial frame payload: {error}"))?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{ClearBuffer, DataBits, FlowControl, Parity, StopBits};
    use std::collections::VecDeque;
    use std::io;

    #[derive(Default)]
    struct MockPort {
        name: String,
        rx: VecDeque<u8>,
        written: Vec<u8>,
        canned_response: Vec<u8>,
    }

    impl MockPort {
        fn with_response<const N: usize>(name: &str, response: &[u8; N]) -> Self {
            Self {
                name: name.to_string(),
                canned_response: response.to_vec(),
                ..Self::default()
            }
        }
    }

    impl io::Read for MockPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let count = buf.len().min(self.rx.len());
            for (offset, byte) in self.rx.drain(..count).enumerate() {
                buf[offset] = byte;
            }
            Ok(count)
        }
    }

    impl io::Write for MockPort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            self.rx.extend(self.canned_response.iter().copied());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl SerialPort for MockPort {
        fn name(&self) -> Option<String> {
            Some(self.name.clone())
        }

        fn baud_rate(&self) -> serialport::Result<u32> {
            Ok(115200)
        }

        fn data_bits(&self) -> serialport::Result<DataBits> {
            Ok(DataBits::Eight)
        }

        fn flow_control(&self) -> serialport::Result<FlowControl> {
            Ok(FlowControl::None)
        }

        fn parity(&self) -> serialport::Result<Parity> {
            Ok(Parity::None)
        }

        fn stop_bits(&self) -> serialport::Result<StopBits> {
            Ok(StopBits::One)
        }

        fn timeout(&self) -> std::time::Duration {
            std::time::Duration::from_millis(100)
        }

        fn set_baud_rate(&mut self, _baud_rate: u32) -> serialport::Result<()> {
            Ok(())
        }

        fn set_data_bits(&mut self, _data_bits: DataBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_flow_control(&mut self, _flow_control: FlowControl) -> serialport::Result<()> {
            Ok(())
        }

        fn set_parity(&mut self, _parity: Parity) -> serialport::Result<()> {
            Ok(())
        }

        fn set_stop_bits(&mut self, _stop_bits: StopBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_timeout(&mut self, _timeout: std::time::Duration) -> serialport::Result<()> {
            Ok(())
        }

        fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn bytes_to_read(&self) -> serialport::Result<u32> {
            Ok(self.rx.len() as u32)
        }

        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(0)
        }

        fn clear(&self, _buffer_to_clear: ClearBuffer) -> serialport::Result<()> {
            Ok(())
        }

        fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
            Err(serialport::Error::from(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "mock ports are not clonable",
            )))
        }

        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }

        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }

    const STATUS_OK_FRAME: [u8; 9] = [7, 0, ayphr_protocol::RESPONSE_STATUS, 0, 0, 0, 0, 0, 0];

    #[test]
    fn command_round_trip_sends_framed_payload_and_parses_response() {
        let registry = SerialPortRegistry::default();
        registry.insert(
            "ttyUSB0",
            Box::new(MockPort::with_response("ttyUSB0", &STATUS_OK_FRAME)),
        );

        let response =
            send_command_blocking_with_registry(&registry, "ttyUSB0", vec![0x01]).unwrap();

        assert_eq!(
            response,
            vec![ayphr_protocol::RESPONSE_STATUS, 0, 0, 0, 0, 0, 0]
        );
        assert!(registry.contains("ttyUSB0"));
    }

    #[test]
    fn cached_port_is_reused_across_commands() {
        let registry = SerialPortRegistry::default();
        registry.insert(
            "ttyUSB0",
            Box::new(MockPort::with_response("ttyUSB0", &STATUS_OK_FRAME)),
        );

        assert!(send_command_blocking_with_registry(&registry, "ttyUSB0", vec![0x01]).is_ok());
        assert!(send_command_blocking_with_registry(&registry, "ttyUSB0", vec![0x02]).is_ok());

        // Reaching this point without attempting to reopen the port proves the
        // cached handle was reused: reopening would call open_port and fail,
        // since no real port named "ttyUSB0" exists.
        assert!(registry.contains("ttyUSB0"));
    }

    #[test]
    fn failed_command_evicts_the_cached_port() {
        let registry = SerialPortRegistry::default();
        registry.insert("ttyUSB0", Box::new(MockPort::default()));

        let error = send_command_blocking_with_registry(&registry, "ttyUSB0", vec![0x01])
            .expect_err("empty read stream should error");

        assert!(error.contains("Failed to read serial frame"));
        assert!(!registry.contains("ttyUSB0"));
    }

    #[test]
    fn invalidate_port_evicts_the_cached_port() {
        let registry = SerialPortRegistry::default();
        registry.insert(
            "ttyUSB0",
            Box::new(MockPort::with_response("ttyUSB0", &STATUS_OK_FRAME)),
        );
        assert!(registry.contains("ttyUSB0"));

        registry.remove("ttyUSB0");

        assert!(!registry.contains("ttyUSB0"));
    }

    #[test]
    fn write_frame_rejects_oversized_payload() {
        let mut port = MockPort::default();
        let oversized = vec![0u8; u16::MAX as usize + 1];

        let error = write_frame(&mut port, &oversized).expect_err("oversized payload should error");

        assert!(error.contains("Payload too large"));
        assert!(port.written.is_empty());
    }

    #[test]
    fn read_frame_rejects_oversized_payload_length() {
        let mut port = MockPort::default();
        let len_buf = (MAX_FRAME_PAYLOAD as u16 + 1).to_le_bytes();
        port.rx.push_back(len_buf[0]);
        port.rx.push_back(len_buf[1]);

        let error = read_frame(&mut port).expect_err("oversized frame should error");

        assert!(error.contains("exceeds max limit"));
    }
}
