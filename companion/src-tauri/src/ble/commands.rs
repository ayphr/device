use btleplug::api::{Central, Peripheral as _, ScanFilter};
use btleplug::platform::Peripheral;
use tauri::{AppHandle, State};
use tokio::time::{sleep, Duration};
use tracing::warn;

use crate::commands;
use crate::protocol::log_string_error;
use crate::transport::Transport;
use crate::types::BleConnectionState;
use ayphr_protocol::{
    BLE_CHUNK_SIZE, COMMAND_AUTHENTICATE, FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID,
    FIRMWARE_TX_CHARACTERISTIC_UUID, RESPONSE_AUTH_FAILED, RESPONSE_AUTH_OK,
};

use super::protocol::parse_uuid;
use super::scanner::get_primary_adapter;
use super::state::{
    connection_lock, emit_devices, get_live_peripheral, refresh_snapshots_with_connection_state,
    remove_connection_if_matches, remove_live_peripheral, upsert_connection,
    upsert_live_peripheral, ActiveBleConnection, BleDeviceStore,
};

const DISCOVERY_RETRY_ATTEMPTS: usize = 5;
const DISCOVERY_RETRY_DELAY: Duration = Duration::from_millis(600);
const FIRMWARE_RECONNECT_ATTEMPTS: usize = 10;
const FIRMWARE_RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[tauri::command]
pub async fn connect_ble_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("connect failed", error, "ble"))?;
    let status = match Transport::Ble(connection.clone()).query_status().await {
        Ok(status) => status,
        Err(error) => {
            invalidate_connection(&device_id, &store, &connection).await;
            return Err(log_string_error("status query failed", error, "ble"));
        }
    };
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    connection.auth_required = status.auth_required;
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

    let response = match super::protocol::send_command(&connection, command).await {
        Ok(response) => response,
        Err(error) => {
            invalidate_connection(&device_id, &store, &connection).await;
            return Err(log_string_error(
                "authenticate command failed",
                error,
                "ble",
            ));
        }
    };
    match response.first().copied() {
        Some(RESPONSE_AUTH_OK) => {
            connection.authenticated = true;
        }
        Some(RESPONSE_AUTH_FAILED) => {
            connection.authenticated = false;
            upsert_connection(&store, &device_id, connection);
            refresh_snapshots_with_connection_state(&store);
            emit_devices(&app, &store);
            return Err(log_string_error(
                "authenticate rejected",
                "Invalid device password",
                "ble",
            ));
        }
        Some(ayphr_protocol::RESPONSE_AUTH_LOCKED) => {
            connection.authenticated = false;
            upsert_connection(&store, &device_id, connection);
            refresh_snapshots_with_connection_state(&store);
            emit_devices(&app, &store);
            return Err(log_string_error(
                "authenticate locked",
                "Too many failed attempts. The device is temporarily locked; try again in a minute.",
                "ble",
            ));
        }
        _ => {
            return Err(log_string_error(
                "authenticate rejected",
                "Unexpected response from device while authenticating",
                "ble",
            ));
        }
    }

    let status = match Transport::Ble(connection.clone()).query_status().await {
        Ok(status) => status,
        Err(error) => {
            invalidate_connection(&device_id, &store, &connection).await;
            return Err(log_string_error(
                "post-auth status query failed",
                error,
                "ble",
            ));
        }
    };
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    connection.auth_required = status.auth_required;
    upsert_connection(&store, &device_id, connection.clone());
    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);

    Ok(commands::build_connection_state(&status))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn submit_ble_setup(
    device_id: String,
    device_name: String,
    wifi_ssid: String,
    wifi_password: String,
    device_password: String,
    auth_required: bool,
    skip_wifi: bool,
    sampling_interval_secs: u32,
    upload_interval_secs: u32,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("setup connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection.clone());

    let result = match commands::do_submit_setup(
        &transport,
        &device_name,
        &wifi_ssid,
        &wifi_password,
        &device_password,
        auth_required,
        skip_wifi,
        sampling_interval_secs,
        upload_interval_secs,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            invalidate_connection(&device_id, &store, &connection).await;
            return Err(error);
        }
    };

    let status = match transport.query_status().await {
        Ok(status) => status,
        Err(error) => {
            invalidate_connection(&device_id, &store, &connection).await;
            return Err(log_string_error(
                "post-setup status query failed",
                error,
                "ble",
            ));
        }
    };
    connection.setup_complete = status.setup_complete;
    connection.authenticated = status.authenticated;
    connection.auth_required = status.auth_required;
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
    let transport = Transport::Ble(connection.clone());
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
    let transport = Transport::Ble(connection.clone());
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
    let transport = Transport::Ble(connection.clone());
    commands::do_change_password(&transport, &current_password, &new_password).await
}

#[tauri::command]
pub async fn rename_ble_device(
    device_id: String,
    name: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("rename connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection.clone());
    commands::do_rename_device(&transport, &name).await?;

    let status = transport
        .query_status()
        .await
        .map_err(|error| log_string_error("post-rename status query failed", error, "ble"))?;
    refresh_ble_snapshot(&app, &store, &device_id, &status);
    Ok(())
}

/// Updates the cached snapshot after a status change (rename, setup, auth)
/// so the device grid reflects the new name/state without waiting for the
/// next scan tick.
fn refresh_ble_snapshot(
    app: &AppHandle,
    store: &BleDeviceStore,
    device_id: &str,
    status: &crate::types::ParsedStatus,
) {
    {
        let mut devices = store.devices.lock().unwrap();
        if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
            device.name = status.device_name.clone();
            device.setup_complete = status.setup_complete;
        }
    }
    refresh_snapshots_with_connection_state(store);
    emit_devices(app, store);
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
pub async fn get_live_data_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<Option<crate::types::LiveDataResult>, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("live data connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_get_live_data(&transport).await
}

#[tauri::command]
pub async fn get_device_settings_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<crate::types::DeviceSettingsResult, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("settings connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_get_device_settings(&transport).await
}

#[tauri::command]
pub async fn update_device_settings_ble(
    device_id: String,
    sampling_interval_secs: u32,
    upload_interval_secs: u32,
    store: State<'_, BleDeviceStore>,
) -> Result<crate::types::DeviceSettingsResult, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("settings connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_update_device_settings(&transport, sampling_interval_secs, upload_interval_secs)
        .await
}

#[tauri::command]
pub async fn start_cloud_link_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<crate::types::CloudLinkResult, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("cloud link connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_start_cloud_link(&transport).await
}

#[tauri::command]
pub async fn get_cloud_link_status_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<crate::types::CloudLinkResult, String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("cloud link connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_cloud_link_status(&transport).await
}

#[tauri::command]
pub async fn unlink_cloud_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("cloud unlink connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_unlink_cloud(&transport).await
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
    let signature = commands::load_signature_for_firmware(&firmware_path)
        .map_err(|error| log_string_error("failed to load firmware signature", error, "ble"))?;
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("firmware update connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection.clone());
    let result = commands::do_update_firmware(
        &transport,
        firmware_data,
        signature,
        &app,
        BLE_CHUNK_SIZE,
        "ble",
    )
    .await;

    if result.is_ok() {
        reconnect_after_firmware_update(&device_id, &app, &store).await;
    } else {
        invalidate_connection(&device_id, &store, &connection).await;
    }

    result
}

#[tauri::command]
pub async fn download_and_update_firmware_ble(
    device_id: String,
    download_url: String,
    expected_sha256: String,
    expected_signature: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("firmware update connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection.clone());
    let result = commands::do_download_and_update_firmware(
        &transport,
        &download_url,
        &expected_sha256,
        &expected_signature,
        &app,
        BLE_CHUNK_SIZE,
        "ble",
    )
    .await;

    if result.is_ok() {
        reconnect_after_firmware_update(&device_id, &app, &store).await;
    } else {
        invalidate_connection(&device_id, &store, &connection).await;
    }

    result
}

#[tauri::command]
pub async fn ota_rollback_ble(
    device_id: String,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let connection = ensure_connected(device_id.clone(), &store)
        .await
        .map_err(|error| log_string_error("OTA rollback connect failed", error, "ble"))?;
    let transport = Transport::Ble(connection);
    commands::do_ota_rollback(&transport).await
}

#[tauri::command]
pub async fn disconnect_ble_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, BleDeviceStore>,
) -> Result<(), String> {
    let device_lock = connection_lock(&store, &device_id);
    let _connection_guard = device_lock.lock().await;

    let connection = {
        let mut guard = store.connections.lock().unwrap();
        guard.remove(&device_id)
    };

    if let Some(active) = connection {
        let _ = active.peripheral.disconnect().await;
    }
    remove_live_peripheral(&store, &device_id);

    refresh_snapshots_with_connection_state(&store);
    emit_devices(&app, &store);
    Ok(())
}

async fn ensure_connected(
    device_id: String,
    store: &BleDeviceStore,
) -> Result<ActiveBleConnection, String> {
    let device_lock = connection_lock(store, &device_id);
    let _connection_guard = device_lock.lock().await;

    let existing = {
        let guard = store.connections.lock().unwrap();
        guard.get(&device_id).cloned()
    };

    if let Some(connection) = existing {
        if connection
            .peripheral
            .is_connected()
            .await
            .map_err(|error| {
                log_string_error("checking existing connection failed", error, "ble")
            })?
        {
            return Ok(connection);
        }

        let mut guard = store.connections.lock().unwrap();
        guard.remove(&device_id);
    }

    let cached_peripheral = get_live_peripheral(store, &device_id);
    if let Some(peripheral) = cached_peripheral {
        match build_connection(device_id.clone(), peripheral, store).await {
            Ok(connection) => {
                upsert_connection(store, &device_id, connection.clone());
                return Ok(connection);
            }
            Err(error) => {
                warn!("[ble] cached peripheral for {device_id} is unusable: {error}");
                remove_live_peripheral(store, &device_id);
            }
        }
    } else {
        warn!("[ble] cached peripheral missing for {device_id}, falling back to rediscovery");
    }

    let peripheral = discover_peripheral(&device_id).await.ok_or_else(|| {
        log_string_error(
            "device lookup failed",
            "Device is not currently discoverable",
            "ble",
        )
    })?;
    let connection = build_connection(device_id.clone(), peripheral.clone(), store)
        .await
        .map_err(|error| log_string_error("failed to prepare BLE connection", error, "ble"))?;
    upsert_live_peripheral(store, &device_id, peripheral);
    upsert_connection(store, &device_id, connection.clone());
    Ok(connection)
}

async fn invalidate_connection(
    device_id: &str,
    store: &BleDeviceStore,
    connection: &ActiveBleConnection,
) {
    let device_lock = connection_lock(store, device_id);
    let _connection_guard = device_lock.lock().await;

    let peripheral = remove_connection_if_matches(store, device_id, &connection.command_lock);
    if peripheral.is_some() {
        refresh_snapshots_with_connection_state(store);
    }
    if let Some(peripheral) = peripheral {
        let _ = peripheral.disconnect().await;
    }
}

async fn build_connection(
    device_id: String,
    peripheral: Peripheral,
    store: &BleDeviceStore,
) -> Result<ActiveBleConnection, String> {
    if !peripheral
        .is_connected()
        .await
        .map_err(|error| format!("checking peripheral connected state failed: {error}"))?
    {
        peripheral
            .connect()
            .await
            .map_err(|error| format!("failed to connect to device: {error}"))?;
    }

    peripheral
        .discover_services()
        .await
        .map_err(|error| format!("failed to discover device services: {error}"))?;

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
            log_string_error(
                "characteristic lookup failed",
                "Missing firmware RX characteristic",
                "ble",
            )
        })?;

    let tx_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == tx_uuid)
        .cloned()
        .ok_or_else(|| {
            log_string_error(
                "characteristic lookup failed",
                "Missing firmware TX characteristic",
                "ble",
            )
        })?;

    let (initial_setup_complete, initial_authenticated, initial_auth_required) = {
        let devices = store.devices.lock().unwrap();
        devices
            .iter()
            .find(|d| d.id == device_id)
            .map(|d| (d.setup_complete, d.authenticated, d.auth_required))
            .unwrap_or((false, false, false))
    };

    peripheral
        .subscribe(&tx_characteristic)
        .await
        .map_err(|error| format!("failed to subscribe to device notifications: {error}"))?;

    Ok(ActiveBleConnection {
        peripheral,
        rx_characteristic,
        tx_characteristic,
        setup_complete: initial_setup_complete,
        authenticated: initial_authenticated,
        auth_required: initial_auth_required,
        command_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
    })
}

async fn reconnect_after_firmware_update(device_id: &str, app: &AppHandle, store: &BleDeviceStore) {
    {
        let mut guard = store.connections.lock().unwrap();
        guard.remove(device_id);
    }
    refresh_snapshots_with_connection_state(store);
    emit_devices(app, store);

    for attempt in 1..=FIRMWARE_RECONNECT_ATTEMPTS {
        if attempt > 1 {
            sleep(FIRMWARE_RECONNECT_DELAY).await;
        }

        let connection = match ensure_connected(device_id.to_string(), store).await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::debug!(
                    "[ble] firmware reconnect attempt {attempt}/{FIRMWARE_RECONNECT_ATTEMPTS} failed: {error}"
                );
                continue;
            }
        };

        let status = match Transport::Ble(connection.clone()).query_status().await {
            Ok(status) => status,
            Err(error) => {
                tracing::debug!(
                    "[ble] firmware reconnect status attempt {attempt}/{FIRMWARE_RECONNECT_ATTEMPTS} failed: {error}"
                );
                invalidate_connection(device_id, store, &connection).await;
                continue;
            }
        };

        let mut connection = connection;
        connection.setup_complete = status.setup_complete;
        connection.authenticated = status.authenticated;
        connection.auth_required = status.auth_required;
        upsert_connection(store, device_id, connection);
        refresh_snapshots_with_connection_state(store);
        emit_devices(app, store);
        tracing::info!("[ble] reconnected after firmware update on attempt {attempt}");
        return;
    }

    tracing::warn!(
        "[ble] device did not reconnect after firmware update after {FIRMWARE_RECONNECT_ATTEMPTS} attempts"
    );
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

        if let Err(error) = adapter.start_scan(ScanFilter::default()).await {
            tracing::warn!("[ble] scan start attempt {} failed: {}", attempt, error);
            sleep(DISCOVERY_RETRY_DELAY).await;
            continue;
        }

        let peripherals = match adapter.peripherals().await {
            Ok(peripherals) => peripherals,
            Err(error) => {
                tracing::warn!(
                    "[ble] peripheral enumeration attempt {} failed: {}",
                    attempt,
                    error
                );
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
                    tracing::warn!(
                        "[ble] property lookup attempt {} failed: {}",
                        attempt,
                        error
                    );
                    continue;
                }
            }
        }
        tracing::info!(
            "[ble] device {} not found on discovery attempt {}",
            device_id,
            attempt
        );
        sleep(DISCOVERY_RETRY_DELAY).await;
    }

    None
}
