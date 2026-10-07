use serialport::{SerialPortInfo, SerialPortType};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tracing::debug;

use super::constants::SERIAL_DEVICES_UPDATED_EVENT;
use super::protocol::query_status;
use super::state::{SerialDeviceSnapshot, SerialDeviceStore};
use crate::constants::{DEVICE_RETENTION_WINDOW, SCAN_INTERVAL};

const NON_DEVICE_PORT_PATTERNS: &[&str] = &[
    "Bluetooth",
    "bluetooth",
    "SOC",
    "soc",
    "debug-console",
    "debug_console",
    "AirPods",
    "WCH",
    "Model",
];

/// USB vendor IDs of the UART bridges found on supported Geo hardware and
/// common dev boards: WCH CH34x, Silicon Labs CP210x, FTDI, Espressif native
/// USB, Arduino and Raspberry Pi RP2040.
const ALLOWED_USB_VENDOR_IDS: &[u16] = &[0x1A86, 0x10C4, 0x0403, 0x303A, 0x2341, 0x2E8A];

/// How long a port that failed a probe is left alone before retrying, so
/// unrelated serial devices are not re-opened on every scan tick.
const PROBE_BACKOFF: Duration = Duration::from_secs(30);

fn is_likely_non_device_port(port_name: &str) -> bool {
    let name = port_name.to_lowercase();
    NON_DEVICE_PORT_PATTERNS
        .iter()
        .any(|pattern| name.contains(&pattern.to_lowercase()))
}

/// Whether a port is safe to probe with a Geo status request on first sight.
/// Non-USB ports and USB devices from unknown vendors are never probed; a
/// port that has already answered a Geo probe stays eligible afterwards.
fn is_probe_candidate(port: &SerialPortInfo) -> bool {
    if is_likely_non_device_port(&port.port_name) {
        return false;
    }

    match &port.port_type {
        SerialPortType::UsbPort(usb) => ALLOWED_USB_VENDOR_IDS.contains(&usb.vid),
        _ => false,
    }
}

pub async fn scan_serial_devices(app: AppHandle, store: SerialDeviceStore) -> Result<(), String> {
    let mut previous_ports: HashSet<String> = HashSet::new();
    let mut seen_devices: HashMap<String, (SerialDeviceSnapshot, Instant)> = HashMap::new();
    let mut known_geo_ports: HashSet<String> = HashSet::new();
    let mut probe_backoff: HashMap<String, Instant> = HashMap::new();

    loop {
        let now = Instant::now();
        let ports = serialport::available_ports().map_err(|error| error.to_string())?;
        let mut present_ports = HashSet::new();

        for port in ports {
            let port_name = port.port_name.clone();
            present_ports.insert(port_name.clone());

            let already_known = known_geo_ports.contains(&port_name);
            if !already_known && !is_probe_candidate(&port) {
                debug!("[serial] skipping non-candidate port: {}", port_name);
                continue;
            }

            if probe_backoff
                .get(&port_name)
                .is_some_and(|failed_at| failed_at.elapsed() < PROBE_BACKOFF)
            {
                debug!("[serial] backing off from port: {}", port_name);
                continue;
            }

            let status = match query_status(&port_name).await {
                Ok(status) => status,
                Err(_) => {
                    probe_backoff.insert(port_name.clone(), Instant::now());
                    continue;
                }
            };
            probe_backoff.remove(&port_name);
            known_geo_ports.insert(port_name.clone());

            let snapshot = SerialDeviceSnapshot {
                id: port_name.clone(),
                name: status.device_name.clone(),
                model_id: "geo-gen1".to_string(),
                transport: "serial".to_string(),
                setup_complete: status.setup_complete,
                address: port_name.clone(),
                rssi: None,
                signal_strength: 0,
                connected: true,
                authenticated: !status.auth_required || status.authenticated,
                auth_required: status.auth_required,
                connectable: true,
                status_label: "Serial connected".to_string(),
                last_seen_seconds_ago: 0,
                tx_power_level: None,
                manufacturer_data: Vec::new(),
                service_uuids: Vec::new(),
            };

            seen_devices.insert(port_name.clone(), (snapshot, now));
        }

        for disappeared in previous_ports.difference(&present_ports) {
            crate::serial::protocol::invalidate_port(disappeared);
        }

        // Forget bookkeeping for ports that are physically gone so a
        // reconnected device goes through candidate screening again.
        known_geo_ports.retain(|port_name| present_ports.contains(port_name));
        probe_backoff.retain(|port_name, _| present_ports.contains(port_name));

        seen_devices.retain(|_, (_, seen_at)| seen_at.elapsed() <= DEVICE_RETENTION_WINDOW);

        let mut active_devices = seen_devices
            .iter_mut()
            .map(|(_, (snapshot, seen_at))| {
                snapshot.last_seen_seconds_ago = seen_at.elapsed().as_secs();
                snapshot.clone()
            })
            .collect::<Vec<_>>();

        active_devices.sort_by(|left, right| left.name.cmp(&right.name));

        *store.devices.lock().unwrap() = active_devices.clone();
        let _ = app.emit(SERIAL_DEVICES_UPDATED_EVENT, active_devices);

        previous_ports = present_ports;
        tokio::time::sleep(SCAN_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;

    fn port(name: &str, port_type: SerialPortType) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type,
        }
    }

    fn usb_port(name: &str, vid: u16) -> SerialPortInfo {
        port(
            name,
            SerialPortType::UsbPort(UsbPortInfo {
                vid,
                pid: 0x7523,
                serial_number: None,
                manufacturer: None,
                product: None,
            }),
        )
    }

    #[test]
    fn allowlisted_usb_vids_are_candidates() {
        for vid in ALLOWED_USB_VENDOR_IDS {
            assert!(
                is_probe_candidate(&usb_port("ttyUSB0", *vid)),
                "vid {vid:#06x}"
            );
        }
    }

    #[test]
    fn unknown_usb_vids_are_not_candidates() {
        assert!(!is_probe_candidate(&usb_port("ttyUSB0", 0x1234)));
    }

    #[test]
    fn denylisted_names_are_never_candidates() {
        for name in [
            "/dev/tty.Bluetooth-Port",
            "/dev/cu.WCHUSBSerial110",
            "/dev/tty.debug-console",
        ] {
            assert!(!is_probe_candidate(&usb_port(name, 0x1A86)), "{name}");
        }
    }

    #[test]
    fn non_usb_ports_are_not_candidates() {
        let pci = port("ttyS0", SerialPortType::PciPort);
        assert!(!is_probe_candidate(&pci));

        let unknown = port("ttyAMA0", SerialPortType::Unknown);
        assert!(!is_probe_candidate(&unknown));

        let bluetooth = port("tty.MFA", SerialPortType::BluetoothPort);
        assert!(!is_probe_candidate(&bluetooth));
    }

    #[test]
    fn known_ports_are_probed_against_the_backoff_window() {
        let now = Instant::now();
        let mut backoff = HashMap::new();
        backoff.insert("ttyUSB0".to_string(), now);

        let blocked = backoff
            .get("ttyUSB0")
            .is_some_and(|failed_at| failed_at.elapsed() < PROBE_BACKOFF);
        assert!(blocked);

        backoff.insert("ttyUSB0".to_string(), now - PROBE_BACKOFF);
        let blocked = backoff
            .get("ttyUSB0")
            .is_some_and(|failed_at| failed_at.elapsed() < PROBE_BACKOFF);
        assert!(!blocked);
    }
}
