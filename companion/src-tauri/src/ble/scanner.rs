use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager as BtManager};
use std::collections::HashMap;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use super::constants::{
    DEVICE_RETENTION_WINDOW, FIRMWARE_MANUFACTURER_ID, SCAN_INTERVAL,
};
use super::state::{upsert_live_peripheral, BleDeviceSnapshot, BleDeviceStore};

pub async fn scan_ble_devices(app: AppHandle, store: BleDeviceStore) -> Result<(), String> {
    let adapter = get_primary_adapter().await?;

    if adapter.start_scan(ScanFilter::default()).await.is_err() {
        let empty_devices = Vec::new();
        *store.devices.lock().unwrap() = empty_devices.clone();
        let _ = app.emit(super::constants::BLE_DEVICES_UPDATED_EVENT, empty_devices);
        return Err("failed to start BLE scan".to_string());
    }

    let mut seen_devices: HashMap<String, (BleDeviceSnapshot, Instant)> = HashMap::new();

    loop {
        let now = Instant::now();
        let peripherals = adapter
            .peripherals()
            .await
            .map_err(|error| error.to_string())?;

        for peripheral in peripherals {
            let Some(properties) = peripheral
                .properties()
                .await
                .map_err(|error| error.to_string())?
            else {
                continue;
            };

            let advertised_service_uuids = properties
                .services
                .iter()
                .map(|service_uuid| service_uuid.to_string())
                .collect::<Vec<_>>();
            let has_firmware_manufacturer_marker = properties
                .manufacturer_data
                .contains_key(&FIRMWARE_MANUFACTURER_ID);

            if !has_firmware_manufacturer_marker {
                continue;
            }

            let peripheral_id = peripheral.id().to_string();
            let address = properties.address.to_string();
            let name = properties
                .local_name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("BLE device {}", peripheral_id));
            let rssi = properties.rssi;
            let signal_strength = signal_strength_from_rssi(rssi);
            let model_id = "geo-mk1".to_string();
            let connectable = properties.rssi.is_some();
            let setup_complete = properties
                .manufacturer_data
                .get(&FIRMWARE_MANUFACTURER_ID)
                .and_then(|payload| payload.first().copied())
                .map(|flag| flag == 1)
                .unwrap_or(false);

            let (is_connected, is_authenticated, known_setup_complete) = {
                let guard = store.connections.lock().unwrap();
                if let Some(connection) = guard.get(&address) {
                    (true, connection.authenticated, connection.setup_complete)
                } else {
                    (false, false, setup_complete)
                }
            };

            let effective_setup_complete = setup_complete || known_setup_complete;
            let authenticated = if effective_setup_complete {
                is_authenticated
            } else {
                is_connected
            };
            let connected = is_connected && (!effective_setup_complete || is_authenticated);
            let manufacturer_data = properties
                .manufacturer_data
                .iter()
                .map(|(company_id, value)| format!("0x{company_id:04x}: {}", format_bytes(value)))
                .collect::<Vec<_>>();
            let service_uuids = advertised_service_uuids;

            let snapshot = BleDeviceSnapshot {
                id: peripheral_id.clone(),
                name,
                address: address.clone(),
                model_id,
                setup_complete: effective_setup_complete,
                rssi,
                signal_strength,
                connected,
                authenticated,
                connectable,
                status_label: status_label(rssi, connectable),
                last_seen_seconds_ago: 0,
                tx_power_level: properties.tx_power_level,
                manufacturer_data,
                service_uuids,
            };

            upsert_live_peripheral(&store, &peripheral_id, peripheral.clone());
            seen_devices.insert(address, (snapshot, now));
        }

        seen_devices.retain(|_, (_, seen_at)| seen_at.elapsed() <= DEVICE_RETENTION_WINDOW);

        let mut active_devices = seen_devices
            .iter_mut()
            .map(|(_, (snapshot, seen_at))| {
                snapshot.last_seen_seconds_ago = seen_at.elapsed().as_secs();
                snapshot.clone()
            })
            .collect::<Vec<_>>();

        active_devices.sort_by(|left, right| right.signal_strength.cmp(&left.signal_strength));

        *store.devices.lock().unwrap() = active_devices.clone();
        let _ = app.emit(super::constants::BLE_DEVICES_UPDATED_EVENT, active_devices);

        tokio::time::sleep(SCAN_INTERVAL).await;
    }
}

pub async fn get_primary_adapter() -> Result<Adapter, String> {
    let manager = BtManager::new().await.map_err(|error| error.to_string())?;
    let adapters = manager
        .adapters()
        .await
        .map_err(|error| error.to_string())?;

    adapters
        .into_iter()
        .next()
        .ok_or_else(|| "no Bluetooth adapters were found".to_string())
}

fn signal_strength_from_rssi(rssi: Option<i16>) -> u8 {
    match rssi {
        Some(value) if value >= -55 => 5,
        Some(value) if value >= -65 => 4,
        Some(value) if value >= -75 => 3,
        Some(value) if value >= -85 => 2,
        Some(_) => 1,
        None => 0,
    }
}

fn status_label(rssi: Option<i16>, connectable: bool) -> String {
    let signal_label = match signal_strength_from_rssi(rssi) {
        5 => "Excellent",
        4 => "Strong",
        3 => "Good",
        2 => "Weak",
        1 => "Very Weak",
        _ => "Unavailable",
    };

    if connectable {
        format!("{signal_label} and connectable")
    } else {
        format!("{signal_label} advertising signal")
    }
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
