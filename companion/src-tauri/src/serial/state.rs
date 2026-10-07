use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::constants::SERIAL_DEVICES_UPDATED_EVENT;

#[derive(Clone, Default)]
pub struct SerialDeviceStore {
    pub devices: Arc<Mutex<Vec<SerialDeviceSnapshot>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialDeviceSnapshot {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub transport: String,
    pub setup_complete: bool,
    pub address: String,
    pub rssi: Option<i16>,
    pub signal_strength: u8,
    pub connected: bool,
    pub authenticated: bool,
    pub auth_required: bool,
    pub connectable: bool,
    pub status_label: String,
    pub last_seen_seconds_ago: u64,
    pub tx_power_level: Option<i16>,
    pub manufacturer_data: Vec<String>,
    pub service_uuids: Vec<String>,
}

/// Replaces the cached snapshot for `device_id` when the port is still
/// listed, so status changes show up without waiting for the next scan tick.
pub fn update_snapshot<F>(store: &SerialDeviceStore, device_id: &str, mutate: F)
where
    F: FnOnce(&mut SerialDeviceSnapshot),
{
    let mut devices = store.devices.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
        mutate(device);
    }
}

pub fn emit_devices(app: &AppHandle, store: &SerialDeviceStore) {
    let snapshot = store.devices.lock().unwrap().clone();
    let _ = app.emit(SERIAL_DEVICES_UPDATED_EVENT, snapshot);
}

pub fn get_serial_devices(store: &SerialDeviceStore) -> Vec<SerialDeviceSnapshot> {
    store.devices.lock().unwrap().clone()
}
