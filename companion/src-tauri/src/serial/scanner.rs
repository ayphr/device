use serialport::SerialPortType;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
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

fn is_likely_non_device_port(port_name: &str) -> bool {
    let name = port_name.to_lowercase();
    NON_DEVICE_PORT_PATTERNS
        .iter()
        .any(|pattern| name.contains(&pattern.to_lowercase()))
}

pub async fn scan_serial_devices(app: AppHandle, store: SerialDeviceStore) -> Result<(), String> {
    let mut previous_ports: HashSet<String> = HashSet::new();

    loop {
        let now = Instant::now();
        let ports = serialport::available_ports().map_err(|error| error.to_string())?;
        let mut seen_devices: HashMap<String, (SerialDeviceSnapshot, Instant)> = HashMap::new();
        let mut seen_ports = HashSet::new();

        let cached_auth: Vec<String> = {
            let mut cache = store.authenticated_cache.lock().unwrap();
            cache.retain(|_, t| t.elapsed() < std::time::Duration::from_secs(600));
            cache.keys().cloned().collect()
        };

        for port in ports {
            let port_name = port.port_name.clone();

            if is_likely_non_device_port(&port_name) {
                debug!("[serial] skipping non-device port: {}", port_name);
                continue;
            }

            let status = match query_status(&port_name) {
                Ok(status) => status,
                Err(_) => continue,
            };
            seen_ports.insert(port_name.clone());

            let cached = cached_auth.contains(&port_name);
            let authenticated = cached || !status.auth_required || status.authenticated;

            let transport_label = match port.port_type {
                SerialPortType::UsbPort(_) => "serial".to_string(),
                _ => "serial".to_string(),
            };

            let snapshot = SerialDeviceSnapshot {
                id: port_name.clone(),
                name: status.device_name.clone(),
                model_id: "geo-gen1".to_string(),
                transport: transport_label,
                setup_complete: status.setup_complete,
                address: port_name.clone(),
                rssi: None,
                signal_strength: 0,
                connected: true,
                authenticated,
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

        for disappeared in previous_ports.difference(&seen_ports) {
            crate::serial::protocol::invalidate_port(disappeared);
        }

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

        previous_ports = seen_ports;
        tokio::time::sleep(SCAN_INTERVAL).await;
    }
}
