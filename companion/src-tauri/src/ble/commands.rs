use btleplug::api::{Central, Peripheral as _};
use btleplug::platform::Peripheral;
use std::fmt::Write as _;
use tauri::{AppHandle, State};
use tokio::time::{sleep, Duration};

use ayphr_protocol::{
    append_field, COMMAND_APPLY_SETUP, COMMAND_AUTHENTICATE, COMMAND_CHANGE_PASSWORD,
    COMMAND_FACTORY_RESET, COMMAND_RESTART, COMMAND_UPDATE_WIFI, RESPONSE_AUTH_FAILED,
    RESPONSE_AUTH_OK, RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_FACTORY_RESET_OK,
    RESPONSE_RESTART_OK, RESPONSE_SETUP_OK, RESPONSE_UPDATE_WIFI_OK,
    FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID, FIRMWARE_TX_CHARACTERISTIC_UUID,
};
use super::protocol::{parse_uuid, query_status, send_command};
use super::scanner::get_primary_adapter;
use super::state::{
    emit_devices, get_live_peripheral, refresh_snapshots_with_connection_state, upsert_connection,
    ActiveBleConnection, BleConnectionState, BleDeviceStore,
};

use tracing::{debug, error, info, warn};

fn log_string_error(context: &str, error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    error!("[ble] {}: {}", context, message);
    message
}

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
        .map_err(|error| log_string_error("connect failed", error))?;
    let status = query_status(&connection)
        .await
        .map_err(|error| log_string_error("status query failed", error))?;
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(BleConnectionState {
        connected: !status.setup_complete || status.authenticated,
        authenticated: status.authenticated,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
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
        .map_err(|error| log_string_error("authenticate connect failed", error))?;
    let mut command = vec![COMMAND_AUTHENTICATE];
    append_field(&mut command, &password)
        .map_err(|error| log_string_error("authenticate payload encoding failed", error))?;

    debug!("[ble] authenticate command bytes={}", format_bytes(&command));

    let response = send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("authenticate command failed", error))?;
    debug!("[ble] authenticate response bytes={}", format_bytes(&response));
    match response.first().copied() {
        Some(RESPONSE_AUTH_OK) => {
            connection.authenticated = true;
        }
        Some(RESPONSE_AUTH_FAILED) => {
            connection.authenticated = false;
            upsert_connection(&store, &device_id, connection);
            refresh_snapshots_with_connection_state(&store);
            emit_devices(&app, &store);
            return Err(log_string_error("authenticate rejected", "Invalid device password"));
        }
        _ => {
            return Err(log_string_error(
                "authenticate rejected",
                "Unexpected response from device while authenticating",
            ));
        }
    }

    let status = query_status(&connection)
        .await
        .map_err(|error| log_string_error("post-auth status query failed", error))?;
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(BleConnectionState {
        connected: !status.setup_complete || status.authenticated,
        authenticated: status.authenticated,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
}

#[tauri::command]
pub async fn submit_ble_setup(
    device_id: String,
    device_name: String,
    wifi_ssid: String,
    wifi_password: String,
    device_password: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("setup connect failed", error))?;
    let mut command = vec![COMMAND_APPLY_SETUP];
    append_field(&mut command, &device_name)
        .map_err(|error| log_string_error("setup name encoding failed", error))?;
    append_field(&mut command, &wifi_ssid)
        .map_err(|error| log_string_error("setup wifi ssid encoding failed", error))?;
    append_field(&mut command, &wifi_password)
        .map_err(|error| log_string_error("setup wifi password encoding failed", error))?;
    append_field(&mut command, &device_password)
        .map_err(|error| log_string_error("setup device password encoding failed", error))?;

    debug!("[ble] setup command bytes={}", format_bytes(&command));

    let response = match send_command(&connection, command).await {
        Ok(response) => response,
        Err(error) => {
            return Err(log_string_error("setup command failed", error));
        }
    };
    debug!("[ble] setup response bytes={}", format_bytes(&response));
    if response.first().copied() != Some(RESPONSE_SETUP_OK) {
        return Err(log_string_error("setup rejected", "Device rejected setup payload"));
    }

    let status = match query_status(&connection).await {
        Ok(status) => status,
        Err(error) => {
            return Err(log_string_error("post-setup status query failed", error));
        }
    };
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(BleConnectionState {
        connected: !status.setup_complete || status.authenticated,
        authenticated: status.authenticated,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
}

#[tauri::command]
pub async fn restart_ble_device(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("restart connect failed", error))?;
    let command = vec![COMMAND_RESTART];

    let response = send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("restart command failed", error))?;

    if response.first().copied() != Some(RESPONSE_RESTART_OK) {
        return Err(log_string_error("restart rejected", "Device rejected restart command"));
    }

    Ok(())
}

#[tauri::command]
pub async fn factory_reset_ble_device(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("factory reset connect failed", error))?;
    let command = vec![COMMAND_FACTORY_RESET];

    let response = send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("factory reset command failed", error))?;

    if response.first().copied() != Some(RESPONSE_FACTORY_RESET_OK) {
        return Err(log_string_error(
            "factory reset rejected",
            "Device rejected factory reset command",
        ));
    }

    Ok(())
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
        .map_err(|error| log_string_error("change password connect failed", error))?;
    let mut command = vec![COMMAND_CHANGE_PASSWORD];
    append_field(&mut command, &current_password)
        .map_err(|error| log_string_error("change password current encoding failed", error))?;
    append_field(&mut command, &new_password)
        .map_err(|error| log_string_error("change password new encoding failed", error))?;

    let response = send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("change password command failed", error))?;

    if response.first().copied() != Some(RESPONSE_CHANGE_PASSWORD_OK) {
        return Err(log_string_error(
            "change password rejected",
            "Device rejected password change",
        ));
    }

    Ok(())
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
        .map_err(|error| log_string_error("update wifi connect failed", error))?;
    let mut command = vec![COMMAND_UPDATE_WIFI];
    append_field(&mut command, &ssid)
        .map_err(|error| log_string_error("update wifi ssid encoding failed", error))?;
    append_field(&mut command, &password)
        .map_err(|error| log_string_error("update wifi password encoding failed", error))?;

    let response = send_command(&connection, command)
        .await
        .map_err(|error| log_string_error("update wifi command failed", error))?;

    if response.first().copied() != Some(RESPONSE_UPDATE_WIFI_OK) {
        return Err(log_string_error(
            "update wifi rejected",
            "Device rejected wifi settings update",
        ));
    }

    Ok(())
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
            .map_err(|error| log_string_error("checking existing connection failed", error))?
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
            log_string_error("device lookup failed", "Device is not currently discoverable")
        })?
    };

    if !peripheral
        .is_connected()
        .await
        .map_err(|error| log_string_error("checking peripheral connected state failed", error))?
    {
        peripheral
            .connect()
            .await
            .map_err(|error| log_string_error("Failed to connect to device", error))?;
    }

    peripheral
        .discover_services()
        .await
        .map_err(|error| log_string_error("Failed to discover device services", error))?;

    let service_uuid = parse_uuid(FIRMWARE_SERVICE_UUID)
        .map_err(|error| log_string_error("service uuid parse failed", error))?;
    let rx_uuid = parse_uuid(FIRMWARE_RX_CHARACTERISTIC_UUID)
        .map_err(|error| log_string_error("rx uuid parse failed", error))?;
    let tx_uuid = parse_uuid(FIRMWARE_TX_CHARACTERISTIC_UUID)
        .map_err(|error| log_string_error("tx uuid parse failed", error))?;

    let characteristics = peripheral.characteristics();
    let has_service = peripheral
        .services()
        .iter()
        .any(|service| service.uuid == service_uuid);
    if !has_service {
        return Err(log_string_error(
            "firmware service missing",
            "Connected device is missing required firmware service",
        ));
    }

    let rx_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == rx_uuid)
        .cloned()
        .ok_or_else(|| {
            log_string_error("characteristic lookup failed", "Missing firmware RX characteristic")
        })?;

    let tx_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == tx_uuid)
        .cloned()
        .ok_or_else(|| {
            log_string_error("characteristic lookup failed", "Missing firmware TX characteristic")
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
                warn!("[ble] adapter lookup attempt {} failed: {}", attempt, error);
                sleep(DISCOVERY_RETRY_DELAY).await;
                continue;
            }
        };

        let peripherals = match adapter.peripherals().await {
            Ok(peripherals) => peripherals,
            Err(error) => {
                warn!("[ble] peripheral enumeration attempt {} failed: {}", attempt, error);
                sleep(DISCOVERY_RETRY_DELAY).await;
                continue;
            }
        };

        for peripheral in peripherals {
            match peripheral.properties().await {
                Ok(Some(_)) => {
                    if peripheral.id().to_string() == device_id {
                        info!("[ble] discovered peripheral on attempt {}", attempt);
                        return Some(peripheral);
                    }
                }
                Ok(None) => continue,
                Err(error) => {
                    warn!("[ble] property lookup attempt {} failed: {}", attempt, error);
                    continue;
                }
            }
        }
        info!("[ble] device {} not found on discovery attempt {}", device_id, attempt);
        sleep(DISCOVERY_RETRY_DELAY).await;
    }

    None
}

fn format_bytes(bytes: &[u8]) -> String {
    let mut output = String::new();

    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }

        let _ = write!(&mut output, "{byte:02x}");
    }

    output
}
