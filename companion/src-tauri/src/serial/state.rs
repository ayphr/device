use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::constants::SERIAL_DEVICES_UPDATED_EVENT;

const AUTH_CACHE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Default)]
pub struct SerialDeviceStore {
    pub devices: Arc<Mutex<Vec<SerialDeviceSnapshot>>>,
    pub authenticated_cache: Arc<Mutex<HashMap<String, Instant>>>,
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

pub fn upsert_auth(store: &SerialDeviceStore, device_id: &str, authenticated: bool) {
    if authenticated {
        let mut cache = store.authenticated_cache.lock().unwrap();
        cache.insert(device_id.to_string(), Instant::now());
    } else {
        let mut cache = store.authenticated_cache.lock().unwrap();
        cache.remove(device_id);
    }
}

pub fn is_authenticated(store: &SerialDeviceStore, device_id: &str) -> bool {
    let cache = store.authenticated_cache.lock().unwrap();
    cache
        .get(device_id)
        .map(|t| t.elapsed() < AUTH_CACHE_TIMEOUT)
        .unwrap_or(false)
}

pub fn refresh_snapshots(store: &SerialDeviceStore) {
    let auth_cache = {
        let mut guard = store.authenticated_cache.lock().unwrap();
        guard.retain(|_, last_auth| last_auth.elapsed() < AUTH_CACHE_TIMEOUT);
        guard.keys().cloned().collect::<Vec<_>>()
    };

    let mut devices = store.devices.lock().unwrap();
    for device in &mut *devices {
        device.authenticated = auth_cache.contains(&device.id);
        device.connected = device.authenticated;
    }
}

pub fn emit_devices(app: &AppHandle, store: &SerialDeviceStore) {
    let snapshot = store.devices.lock().unwrap().clone();
    let _ = app.emit(SERIAL_DEVICES_UPDATED_EVENT, snapshot);
}

pub fn get_serial_devices(store: &SerialDeviceStore) -> Vec<SerialDeviceSnapshot> {
    store.devices.lock().unwrap().clone()
}
