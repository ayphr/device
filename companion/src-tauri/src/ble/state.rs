use btleplug::api::Characteristic;
use btleplug::platform::Peripheral;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

use super::constants::BLE_DEVICES_UPDATED_EVENT;

const AUTH_CACHE_TIMEOUT: Duration = Duration::from_secs(600); // 10 minutes

#[derive(Clone, Default)]
pub struct BleDeviceStore {
    pub devices: Arc<Mutex<Vec<BleDeviceSnapshot>>>,
    pub live_peripherals: Arc<Mutex<HashMap<String, Peripheral>>>,
    pub connections: Arc<Mutex<HashMap<String, ActiveBleConnection>>>,
    pub authenticated_cache: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(Clone)]
pub struct ActiveBleConnection {
    pub peripheral: Peripheral,
    pub rx_characteristic: Characteristic,
    pub tx_characteristic: Characteristic,
    pub setup_complete: bool,
    pub authenticated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleDeviceSnapshot {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub setup_complete: bool,
    pub address: String,
    pub rssi: Option<i16>,
    pub signal_strength: u8,
    pub connected: bool,
    pub authenticated: bool,
    pub connectable: bool,
    pub status_label: String,
    pub last_seen_seconds_ago: u64,
    pub tx_power_level: Option<i16>,
    pub manufacturer_data: Vec<String>,
    pub service_uuids: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleConnectionState {
    pub connected: bool,
    pub authenticated: bool,
    pub setup_complete: bool,
    pub device_name: String,
}

#[derive(Clone)]
pub struct ParsedStatus {
    pub setup_complete: bool,
    pub authenticated: bool,
    pub device_name: String,
}

#[tauri::command]
pub fn get_ble_devices(store: State<'_, BleDeviceStore>) -> Vec<BleDeviceSnapshot> {
    store.devices.lock().unwrap().clone()
}

pub fn upsert_connection(store: &BleDeviceStore, device_id: &str, connection: ActiveBleConnection) {
    if connection.authenticated {
        let mut cache_guard = store.authenticated_cache.lock().unwrap();
        cache_guard.insert(device_id.to_string(), Instant::now());
    }

    let mut guard = store.connections.lock().unwrap();
    guard.insert(device_id.to_string(), connection);
}

pub fn upsert_live_peripheral(store: &BleDeviceStore, device_id: &str, peripheral: Peripheral) {
    let mut guard = store.live_peripherals.lock().unwrap();
    guard.insert(device_id.to_string(), peripheral);
}

pub fn get_live_peripheral(store: &BleDeviceStore, device_id: &str) -> Option<Peripheral> {
    let guard = store.live_peripherals.lock().unwrap();
    guard.get(device_id).cloned()
}

pub fn refresh_snapshots_with_connection_state(store: &BleDeviceStore) {
    let connection_state = {
        let guard = store.connections.lock().unwrap();
        guard
            .iter()
            .map(|(address, connection)| {
                (
                    address.clone(),
                    (connection.authenticated, connection.setup_complete, true),
                )
            })
            .collect::<HashMap<_, _>>()
    };

    let auth_cache = {
        let mut guard = store.authenticated_cache.lock().unwrap();
        guard.retain(|_, last_auth| last_at_elapsed(last_auth) < AUTH_CACHE_TIMEOUT);
        guard.keys().cloned().collect::<Vec<_>>()
    };

    let mut devices = store.devices.lock().unwrap();
    for device in &mut *devices {
        let cached_auth = auth_cache.contains(&device.id);

        if let Some((authenticated, setup_complete, connected)) = connection_state.get(&device.id) {
            device.setup_complete = device.setup_complete || *setup_complete;
            let effective_auth = *authenticated || cached_auth;
            device.authenticated = effective_auth;
            device.connected = *connected || effective_auth;
            continue;
        }

        device.authenticated = cached_auth;
        device.connected = cached_auth;
    }
}

fn last_at_elapsed(at: &Instant) -> Duration {
    at.elapsed()
}

pub fn emit_devices(app: &AppHandle, store: &BleDeviceStore) {
    let snapshot = store.devices.lock().unwrap().clone();
    let _ = app.emit(BLE_DEVICES_UPDATED_EVENT, snapshot);
}
