use btleplug::api::Characteristic;
use btleplug::platform::Peripheral;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

use super::constants::BLE_DEVICES_UPDATED_EVENT;

#[derive(Clone, Default)]
pub struct BleDeviceStore {
    pub devices: Arc<Mutex<Vec<BleDeviceSnapshot>>>,
    pub live_peripherals: Arc<Mutex<HashMap<String, Peripheral>>>,
    pub connections: Arc<Mutex<HashMap<String, ActiveBleConnection>>>,
}

#[derive(Clone)]
pub struct ActiveBleConnection {
    pub peripheral: Peripheral,
    pub rx_characteristic: Characteristic,
    pub tx_characteristic: Characteristic,
    pub setup_complete: bool,
    pub authenticated: bool,
    pub command_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleDeviceSnapshot {
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
    pub connectable: bool,
    pub status_label: String,
    pub last_seen_seconds_ago: u64,
    pub tx_power_level: Option<i16>,
    pub manufacturer_data: Vec<String>,
    pub service_uuids: Vec<String>,
}

#[tauri::command]
pub fn get_ble_devices(store: State<'_, BleDeviceStore>) -> Vec<BleDeviceSnapshot> {
    store.devices.lock().unwrap().clone()
}

pub fn upsert_connection(store: &BleDeviceStore, device_id: &str, connection: ActiveBleConnection) {
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

    let mut devices = store.devices.lock().unwrap();
    for device in &mut *devices {
        if let Some((authenticated, setup_complete, connected)) = connection_state.get(&device.id) {
            device.setup_complete = device.setup_complete || *setup_complete;
            device.authenticated = *authenticated;
            device.connected = *connected;
        } else {
            device.authenticated = false;
            device.connected = false;
        }
    }
}

pub fn emit_devices(app: &AppHandle, store: &BleDeviceStore) {
    let devices = store.devices.lock().unwrap();
    let _ = app.emit(BLE_DEVICES_UPDATED_EVENT, &*devices);
}
