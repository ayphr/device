use tauri::{AppHandle, Emitter, State};

use crate::commands;
use crate::protocol::log_string_error;
use crate::transport::Transport;
use crate::types::{BleConnectionState, FirmwareInfoResult};
use super::constants::SERIAL_DEVICES_UPDATED_EVENT;

use super::protocol::query_status;
use super::state::{emit_devices, SerialDeviceSnapshot, SerialDeviceStore};

#[tauri::command]
pub fn get_serial_devices(store: State<'_, SerialDeviceStore>) -> Vec<SerialDeviceSnapshot> {
    super::state::get_serial_devices(&store)
}

#[tauri::command]
pub async fn connect_serial_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, SerialDeviceStore>,
) -> Result<BleConnectionState, String> {
    let status = query_status(&device_id)
        .map_err(|error| log_string_error("status query failed", error, "serial"))?;

    let mut devices = store.devices.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
        device.setup_complete = status.setup_complete;
        device.name = status.device_name.clone();
        device.authenticated = true;
        device.auth_required = false;
    }
    let _ = app.emit(SERIAL_DEVICES_UPDATED_EVENT, devices.clone());

    Ok(BleConnectionState {
        connected: true,
        authenticated: true,
        auth_required: false,
        wifi_required: status.wifi_required,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
}

#[tauri::command]
pub async fn submit_serial_setup(
    device_id: String,
    device_name: String,
    wifi_ssid: String,
    wifi_password: String,
    device_password: String,
    auth_required: bool,
    skip_wifi: bool,
    app: AppHandle,
    store: State<'_, SerialDeviceStore>,
) -> Result<BleConnectionState, String> {
    let transport = Transport::Serial(device_id.clone());
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
    emit_devices(&app, &store);
    Ok(result)
}

#[tauri::command]
pub async fn restart_serial_device(
    device_id: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_restart(&transport).await
}

#[tauri::command]
pub async fn factory_reset_serial_device(
    device_id: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_factory_reset(&transport).await
}

#[tauri::command]
pub async fn change_serial_device_password(
    device_id: String,
    current_password: String,
    new_password: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_change_password(&transport, &current_password, &new_password).await
}

#[tauri::command]
pub async fn update_serial_device_wifi(
    device_id: String,
    ssid: String,
    password: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_update_wifi(&transport, &ssid, &password).await
}

#[tauri::command]
pub async fn get_firmware_info_serial(
    device_id: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<FirmwareInfoResult, String> {
    let transport = Transport::Serial(device_id);
    commands::do_get_firmware_info(&transport).await
}

#[tauri::command]
pub async fn update_firmware_serial(
    device_id: String,
    firmware_path: String,
    app: AppHandle,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let firmware_data = std::fs::read(&firmware_path)
        .map_err(|error| log_string_error("failed to read firmware file", error, "serial"))?;
    let transport = Transport::Serial(device_id);
    commands::do_update_firmware(
        &transport,
        firmware_data,
        &app,
        ayphr_protocol::SERIAL_CHUNK_SIZE,
        "serial",
    )
    .await
}

#[tauri::command]
pub async fn download_and_update_firmware_serial(
    device_id: String,
    download_url: String,
    app: AppHandle,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_download_and_update_firmware(
        &transport,
        &download_url,
        &app,
        ayphr_protocol::SERIAL_CHUNK_SIZE,
        "serial",
    )
    .await
}
