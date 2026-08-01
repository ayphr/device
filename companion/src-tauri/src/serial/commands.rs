use tauri::{AppHandle, Emitter, State};

use ayphr_protocol::{
    append_field, COMMAND_APPLY_SETUP, COMMAND_CHANGE_PASSWORD, COMMAND_FACTORY_RESET,
    COMMAND_RESTART, COMMAND_UPDATE_WIFI, RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_FACTORY_RESET_OK,
    RESPONSE_RESTART_OK, RESPONSE_SETUP_OK, RESPONSE_UPDATE_WIFI_OK,
};

use crate::ble::state::BleConnectionState;

use super::protocol::{query_status, send_command};
use super::state::{get_serial_devices as get_serial_devices_impl, SerialDeviceStore, SerialDeviceSnapshot};

#[tauri::command]
pub fn get_serial_devices(store: State<'_, SerialDeviceStore>) -> Vec<SerialDeviceSnapshot> {
    get_serial_devices_impl(&store)
}

fn log_string_error(context: &str, error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    tracing::error!("[serial] {}: {}", context, message);
    message
}

#[tauri::command]
pub async fn connect_serial_device(
    device_id: String,
    app: AppHandle,
    store: State<'_, SerialDeviceStore>,
) -> Result<BleConnectionState, String> {
    let status = query_status(&device_id).map_err(|error| log_string_error("status query failed", error))?;
    let connected = true;

    let mut devices = store.devices.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
        device.setup_complete = status.setup_complete;
        device.name = status.device_name.clone();
        device.authenticated = true;
        device.auth_required = false;
    }
    let _ = app.emit(super::constants::SERIAL_DEVICES_UPDATED_EVENT, devices.clone());

    Ok(BleConnectionState {
        connected,
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
    let mut command = vec![COMMAND_APPLY_SETUP];
    append_field(&mut command, &device_name)
        .map_err(|error| log_string_error("setup name encoding failed", error))?;
    append_field(&mut command, &wifi_ssid)
        .map_err(|error| log_string_error("setup wifi ssid encoding failed", error))?;
    append_field(&mut command, &wifi_password)
        .map_err(|error| log_string_error("setup wifi password encoding failed", error))?;
    append_field(&mut command, &device_password)
        .map_err(|error| log_string_error("setup device password encoding failed", error))?;
    command.push(if auth_required { 1 } else { 0 });
    command.push(if skip_wifi { 1 } else { 0 });

    let response = send_command(&device_id, command)
        .map_err(|error| log_string_error("setup command failed", error))?;

    if response.first().copied() != Some(RESPONSE_SETUP_OK) {
        return Err(log_string_error("setup rejected", "Device rejected setup payload"));
    }

    let status = query_status(&device_id)
        .map_err(|error| log_string_error("post-setup status query failed", error))?;

    let mut devices = store.devices.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
        device.setup_complete = status.setup_complete;
        device.name = status.device_name.clone();
        device.authenticated = true;
        device.auth_required = false;
    }
    let _ = app.emit(super::constants::SERIAL_DEVICES_UPDATED_EVENT, devices.clone());

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
pub async fn restart_serial_device(device_id: String) -> Result<(), String> {
    let response = send_command(&device_id, vec![COMMAND_RESTART])
        .map_err(|error| log_string_error("restart command failed", error))?;
    if response.first().copied() != Some(RESPONSE_RESTART_OK) {
        return Err(log_string_error("restart rejected", "Device rejected restart command"));
    }
    Ok(())
}

#[tauri::command]
pub async fn factory_reset_serial_device(device_id: String) -> Result<(), String> {
    let response = send_command(&device_id, vec![COMMAND_FACTORY_RESET])
        .map_err(|error| log_string_error("factory reset command failed", error))?;
    if response.first().copied() != Some(RESPONSE_FACTORY_RESET_OK) {
        return Err(log_string_error("factory reset rejected", "Device rejected factory reset command"));
    }
    Ok(())
}

#[tauri::command]
pub async fn change_serial_device_password(
    device_id: String,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    let mut command = vec![COMMAND_CHANGE_PASSWORD];
    append_field(&mut command, &current_password)
        .map_err(|error| log_string_error("change password current encoding failed", error))?;
    append_field(&mut command, &new_password)
        .map_err(|error| log_string_error("change password new encoding failed", error))?;

    let response = send_command(&device_id, command)
        .map_err(|error| log_string_error("change password command failed", error))?;

    if response.first().copied() != Some(RESPONSE_CHANGE_PASSWORD_OK) {
        return Err(log_string_error("change password rejected", "Device rejected password change"));
    }

    Ok(())
}

#[tauri::command]
pub async fn update_serial_device_wifi(
    device_id: String,
    ssid: String,
    password: String,
) -> Result<(), String> {
    let mut command = vec![COMMAND_UPDATE_WIFI];
    append_field(&mut command, &ssid)
        .map_err(|error| log_string_error("update wifi ssid encoding failed", error))?;
    append_field(&mut command, &password)
        .map_err(|error| log_string_error("update wifi password encoding failed", error))?;

    let response = send_command(&device_id, command)
        .map_err(|error| log_string_error("update wifi command failed", error))?;

    if response.first().copied() != Some(RESPONSE_UPDATE_WIFI_OK) {
        return Err(log_string_error("update wifi rejected", "Device rejected wifi settings update"));
    }

    Ok(())
}
