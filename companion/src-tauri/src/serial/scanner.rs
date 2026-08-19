use serialport::SerialPortType;
use std::collections::HashMap;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use super::constants::{DEVICE_RETENTION_WINDOW, SCAN_INTERVAL};
use super::protocol::query_status;
use super::state::{SerialDeviceSnapshot, SerialDeviceStore};

pub async fn scan_serial_devices(app: AppHandle, store: SerialDeviceStore) -> Result<(), String> {
    let mut seen_devices: HashMap<String, (SerialDeviceSnapshot, Instant)> = HashMap::new();

    loop {
        let now = Instant::now();
        let ports = serialport::available_ports().map_err(|error| error.to_string())?;

        for port in ports {
            let port_name = port.port_name.clone();
            let port_name_task = port_name.clone();

            let status = match tokio::task::spawn_blocking(move || query_status(&port_name_task)).await {
                Ok(Ok(status)) => status,
                _ => continue,
            };

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
                authenticated: true,
                auth_required: false,
                connectable: true,
                status_label: "Serial connected".to_string(),
                last_seen_seconds_ago: 0,
                tx_power_level: None,
                manufacturer_data: Vec::new(),
                service_uuids: Vec::new(),
            };

            seen_devices.insert(port_name, (snapshot, now));
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
        let _ = app.emit(super::constants::SERIAL_DEVICES_UPDATED_EVENT, active_devices);

        tokio::time::sleep(SCAN_INTERVAL).await;
    }
}
