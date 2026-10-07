use tauri::{AppHandle, State};

use crate::commands;
use crate::protocol::log_string_error;
use crate::transport::Transport;
use crate::types::{BleConnectionState, FirmwareInfoResult};
use ayphr_protocol::{COMMAND_AUTHENTICATE, RESPONSE_AUTH_FAILED, RESPONSE_AUTH_OK};

use super::protocol::query_status;
use super::state::{emit_devices, update_snapshot, SerialDeviceSnapshot, SerialDeviceStore};

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
        .await
        .map_err(|error| log_string_error("status query failed", error, "serial"))?;

    // Serial access is treated as authenticated physical access, so the
    // device only reports auth as required when it was configured with it.
    let authenticated = !status.auth_required || status.authenticated;

    update_snapshot(&store, &device_id, |device| {
        device.setup_complete = status.setup_complete;
        device.name = status.device_name.clone();
        device.authenticated = authenticated;
        device.auth_required = status.auth_required;
        device.connected = true;
    });
    emit_devices(&app, &store);

    Ok(BleConnectionState {
        connected: true,
        authenticated,
        auth_required: status.auth_required,
        wifi_required: status.wifi_required,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
}

#[tauri::command]
pub async fn authenticate_serial_device(
    device_id: String,
    password: String,
    app: AppHandle,
    store: State<'_, SerialDeviceStore>,
) -> Result<BleConnectionState, String> {
    let mut command = vec![COMMAND_AUTHENTICATE];
    ayphr_protocol::append_field(&mut command, &password).map_err(|error| {
        log_string_error("authenticate payload encoding failed", error, "serial")
    })?;

    let transport = Transport::Serial(device_id.clone());
    let response = transport
        .send_command(command)
        .await
        .map_err(|error| log_string_error("authenticate command failed", error, "serial"))?;

    match response.first().copied() {
        Some(RESPONSE_AUTH_OK) => {}
        Some(RESPONSE_AUTH_FAILED) => {
            update_snapshot(&store, &device_id, |device| {
                device.authenticated = false;
            });
            emit_devices(&app, &store);
            return Err(log_string_error(
                "authenticate rejected",
                "Invalid device password",
                "serial",
            ));
        }
        Some(ayphr_protocol::RESPONSE_AUTH_LOCKED) => {
            update_snapshot(&store, &device_id, |device| {
                device.authenticated = false;
            });
            emit_devices(&app, &store);
            return Err(log_string_error(
                "authenticate locked",
                "Too many failed attempts. The device is temporarily locked; try again in a minute.",
                "serial",
            ));
        }
        _ => {
            return Err(log_string_error(
                "authenticate rejected",
                "Unexpected response from device while authenticating",
                "serial",
            ));
        }
    }

    let status = transport
        .query_status()
        .await
        .map_err(|error| log_string_error("post-auth status query failed", error, "serial"))?;

    update_snapshot(&store, &device_id, |device| {
        device.setup_complete = status.setup_complete;
        device.authenticated = true;
        device.auth_required = status.auth_required;
        device.connected = true;
        device.name = status.device_name.clone();
    });
    emit_devices(&app, &store);

    Ok(BleConnectionState {
        connected: true,
        authenticated: true,
        auth_required: status.auth_required,
        wifi_required: status.wifi_required,
        setup_complete: status.setup_complete,
        device_name: status.device_name,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
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

    update_snapshot(&store, &device_id, |device| {
        device.name = result.device_name.clone();
        device.setup_complete = result.setup_complete;
        device.authenticated = result.authenticated;
        device.auth_required = result.auth_required;
    });
    emit_devices(&app, &store);
    Ok(result)
}

#[tauri::command]
pub async fn rename_serial_device(
    device_id: String,
    name: String,
    app: AppHandle,
    store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id.clone());
    commands::do_rename_device(&transport, &name).await?;

    let status = query_status(&device_id)
        .await
        .map_err(|error| log_string_error("post-rename status query failed", error, "serial"))?;

    update_snapshot(&store, &device_id, |device| {
        device.name = status.device_name.clone();
    });
    emit_devices(&app, &store);
    Ok(())
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
    expected_sha256: String,
    app: AppHandle,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_download_and_update_firmware(
        &transport,
        &download_url,
        &expected_sha256,
        &app,
        ayphr_protocol::SERIAL_CHUNK_SIZE,
        "serial",
    )
    .await
}

#[tauri::command]
pub async fn ota_rollback_serial(
    device_id: String,
    _store: State<'_, SerialDeviceStore>,
) -> Result<(), String> {
    let transport = Transport::Serial(device_id);
    commands::do_ota_rollback(&transport).await
}
