use btleplug::api::{Central, Peripheral as _};
use btleplug::platform::Peripheral;
use tauri::{AppHandle, State};
use tokio::time::{sleep, Duration};
use tracing::warn;

use ayphr_protocol::{
    COMMAND_AUTHENTICATE, RESPONSE_AUTH_FAILED, RESPONSE_AUTH_OK,
    FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID, FIRMWARE_TX_CHARACTERISTIC_UUID,
    BLE_CHUNK_SIZE,
};
use crate::commands;
use crate::protocol::log_string_error;
use crate::transport::Transport;
use crate::types::BleConnectionState;

use super::protocol::parse_uuid;
use super::scanner::get_primary_adapter;
use super::state::{
    emit_devices, get_live_peripheral, refresh_snapshots_with_connection_state, upsert_connection,
    ActiveBleConnection, BleDeviceStore,
};

const DISCOVERY_RETRY_ATTEMPTS: usize = 5;
const DISCOVERY_RETRY_DELAY: Duration = Duration::from_millis(600);

#[tauri::command]
pub async fn connect_ble_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("connect failed", error, "ble"))?;
    let status = Transport::Ble(connection.clone())
        .query_status()
        .await
        .map_err(|error| log_string_error("status query failed", error, "ble"))?;
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(commands::build_connection_state(&status))
}

#[tauri::command]
pub async fn authenticate_ble_device(
    device_id: String,
    password: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("authenticate connect failed", error, "ble"))?;
    let mut command = vec![COMMAND_AUTHENTICATE];
    ayphr_protocol::append_field(&mut command, &password)
        .map_err(|error| log_string_error("authenticate payload encoding failed", error, "ble"))?;

    let response = super::protocol::send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("authenticate command failed", error, "ble"))?;
    match response.first().copied() {
        Some(RESPONSE_AUTH_OK) => {
            connection.authenticated = true;
        }
        Some(RESPONSE_AUTH_FAILED) => {
            connection.authenticated = false;
            upsert_connection(&store, &device_id, connection);
            refresh_snapshots_with_connection_state(&store);
            emit_devices(&app, &store);
            return Err(log_string_error("authenticate rejected", "Invalid device password", "ble"));
        }
        _ => {
            return Err(log_string_error(
                "authenticate rejected",
                "Unexpected response from device while authenticating",
                "ble",
            ));
        }
    }

    let status = Transport::Ble(connection.clone())
        .query_status()
        .await
        .map_err(|error| log_string_error("post-auth status query failed", error, "ble"))?;
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(commands::build_connection_state(&status))
}

#[tauri::command]
pub async fn submit_ble_setup(
    device_id: String,
    device_name: String,
    wifi_ssid: String,
    wifi_password: String,
    device_password: String,
    auth_required: bool,
    skip_wifi: bool,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("setup connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection.clone());

    let result = commands::do_submit_setup(
        &transport,
        &device_name,
        &wifi_ssid,
        &wifi_password,
        &device_password,
        auth_required,
        skip_wifi,
    )
    .await?;

    let status = transport
        .query_status()
        .await
        .map_err(|error| log_string_error("post-setup status query failed", error, "ble"))?;
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(result)
}

#[tauri::command]
pub async fn restart_ble_device(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("restart connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_restart(&transport).await
}

#[tauri::command]
pub async fn factory_reset_ble_device(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("factory reset connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_factory_reset(&transport).await
}

#[tauri::command]
pub async fn change_ble_device_password(
    device_id: String,
    current_password: String,
    new_password: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("change password connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_change_password(&transport, &current_password, &new_password).await
}

#[tauri::command]
pub async fn update_ble_device_wifi(
    device_id: String,
    ssid: String,
    password: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("update wifi connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_update_wifi(&transport, &ssid, &password).await
}

#[tauri::command]
pub async fn get_firmware_info_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<crate::types::FirmwareInfoResult, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("firmware info connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_get_firmware_info(&transport).await
}

#[tauri::command]
pub async fn update_firmware_ble(
    device_id: String,
    firmware_path: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let firmware_data = std::fs::read(&firmware_path)
        .map_err(|error| log_string_error("failed to read firmware file", error, "ble"))?;
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("firmware update connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_update_firmware(&transport, firmware_data, &app, BLE_CHUNK_SIZE, "ble").await
}

#[tauri::command]
pub async fn download_and_update_firmware_ble(
    device_id: String,
    download_url: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("firmware update connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_download_and_update_firmware(&transport, &download_url, &app, BLE_CHUNK_SIZE, "ble").await
}

#[tauri::command]
pub async fn disconnect_ble_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = {
        let mut guard = store.connections.lock().unwrap();
        guard.remove(&device_id)
    };

    if let Some(active) = connection {
        let _ = active.peripheral.disconnect().await;
    }

    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);
    Ok(())
}

async fn ensure_connected(
    device_id: String,
    store: &BleDeviceStore,
) -> Result<ActiveBleConnection, String> {
    let existing = {
        let guard = store.connections.lock().unwrap();
        guard.get(&device_id).cloned()
    };

    if let Some(connection) = existing {
        if connection
            .peripheral
            .is_connected()
            .await
            .map_err(|error| log_string_error("checking existing connection failed", error, "ble"))?
        {
            return Ok(connection);
        }

        let mut guard = store.connections.lock().unwrap();
        guard.remove(&device_id);
    }

    let peripheral = if let Some(peripheral) = get_live_peripheral(store, &device_id) {
        peripheral
    } else {
        warn!("[ble] cached peripheral missing for {} , falling back to rediscovery", device_id);
        discover_peripheral(&device_id).await.ok_or_else(|| {
            log_string_error("device lookup failed", "Device is not currently discoverable", "ble")
        })?
    };

    if !peripheral
        .is_connected()
        .await
        .map_err(|error| log_string_error("checking peripheral connected state failed", error, "ble"))?
    {
        peripheral
            .connect()
            .await
            .map_err(|error| log_string_error("Failed to connect to device", error, "ble"))?;
    }

    peripheral
        .discover_services()
        .await
        .map_err(|error| log_string_error("Failed to discover device services", error, "ble"))?;

    let service_uuid = parse_uuid(FIRMWARE_SERVICE_UUID)
        .map_err(|error| log_string_error("service uuid parse failed", error, "ble"))?;
    let rx_uuid = parse_uuid(FIRMWARE_RX_CHARACTERISTIC_UUID)
        .map_err(|error| log_string_error("rx uuid parse failed", error, "ble"))?;
    let tx_uuid = parse_uuid(FIRMWARE_TX_CHARACTERISTIC_UUID)
        .map_err(|error| log_string_error("tx uuid parse failed", error, "ble"))?;

    let characteristics = peripheral.characteristics();
    let has_service = peripheral
        .services()
        .iter()
        .any(|service| service.uuid == service_uuid);
    if !has_service {
        return Err(log_string_error(
            "firmware service missing",
            "Connected device is missing required firmware service",
            "ble",
        ));
    }

    let rx_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == rx_uuid)
        .cloned()
        .ok_or_else(|| {
            log_string_error("characteristic lookup failed", "Missing firmware RX characteristic", "ble")
        })?;

    let tx_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == tx_uuid)
        .cloned()
        .ok_or_else(|| {
            log_string_error("characteristic lookup failed", "Missing firmware TX characteristic", "ble")
        })?;

    let authenticated = {
        let guard = store.authenticated_cache.lock().unwrap();
        guard.contains_key(&device_id)
    };

    Ok(ActiveBleConnection {
        peripheral,
        rx_characteristic,
        tx_characteristic,
        setup_complete: true,
        authenticated,
    })
}

async fn discover_peripheral(device_id: &str) -> Option<Peripheral> {
    for attempt in 1..=DISCOVERY_RETRY_ATTEMPTS {
        let adapter = match get_primary_adapter().await {
            Ok(adapter) => adapter,
            Err(error) => {
                tracing::warn!("[ble] adapter lookup attempt {} failed: {}", attempt, error);
                sleep(DISCOVERY_RETRY_DELAY).await;
                continue;
            }
        };

        let peripherals = match adapter.peripherals().await {
            Ok(peripherals) => peripherals,
            Err(error) => {
                tracing::warn!("[ble] peripheral enumeration attempt {} failed: {}", attempt, error);
                sleep(DISCOVERY_RETRY_DELAY).await;
                continue;
            }
        };

        for peripheral in peripherals {
            match peripheral.properties().await {
                Ok(Some(_)) => {
                    if peripheral.id().to_string() == device_id {
                        tracing::info!("[ble] discovered peripheral on attempt {}", attempt);
                        return Some(peripheral);
                    }
                }
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!("[ble] property lookup attempt {} failed: {}", attempt, error);
                    continue;
                }
            }
        }
        tracing::info!("[ble] device {} not found on discovery attempt {}", device_id, attempt);
        sleep(DISCOVERY_RETRY_DELAY).await;
    }

    None
}
