use ayphr_protocol::{
    append_field, COMMAND_APPLY_SETUP, COMMAND_CHANGE_PASSWORD, COMMAND_FACTORY_RESET,
    COMMAND_OTA_BEGIN, COMMAND_OTA_DATA, COMMAND_OTA_END, COMMAND_RESTART, COMMAND_UPDATE_WIFI,
    RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_FACTORY_RESET_OK, RESPONSE_OTA_BEGIN_OK,
    RESPONSE_OTA_DATA_OK, RESPONSE_OTA_END_OK, RESPONSE_RESTART_OK,
    RESPONSE_SETUP_OK, RESPONSE_UPDATE_WIFI_OK,
};
use tauri::{AppHandle, Emitter};

use crate::protocol::{log_string_error, format_bytes};
use crate::transport::Transport;
use crate::types::{
    BleConnectionState, FirmwareInfoResult, FirmwareUpdateProgress, ParsedStatus,
};

pub fn build_connection_state(status: &ParsedStatus) -> BleConnectionState {
    BleConnectionState {
        connected: !status.setup_complete || status.authenticated,
        authenticated: status.authenticated,
        auth_required: status.auth_required,
        wifi_required: status.wifi_required,
        setup_complete: status.setup_complete,
        device_name: status.device_name.clone(),
    }
}

pub async fn do_submit_setup(
    transport: &Transport,
    device_name: &str,
    wifi_ssid: &str,
    wifi_password: &str,
    device_password: &str,
    auth_required: bool,
    skip_wifi: bool,
) -> Result<BleConnectionState, String> {
    let mut command = vec![COMMAND_APPLY_SETUP];
    append_field(&mut command, device_name)
        .map_err(|e| log_string_error("setup name encoding failed", e, "shared"))?;
    append_field(&mut command, wifi_ssid)
        .map_err(|e| log_string_error("setup wifi ssid encoding failed", e, "shared"))?;
    append_field(&mut command, wifi_password)
        .map_err(|e| log_string_error("setup wifi password encoding failed", e, "shared"))?;
    append_field(&mut command, device_password)
        .map_err(|e| log_string_error("setup device password encoding failed", e, "shared"))?;
    command.push(if auth_required { 1 } else { 0 });
    command.push(if skip_wifi { 1 } else { 0 });

    tracing::debug!("[shared] setup command bytes={}", format_bytes(&command));

    let response = transport
        .send_command(command)
        .await
        .map_err(|e| log_string_error("setup command failed", e, "shared"))?;
    tracing::debug!("[shared] setup response bytes={}", format_bytes(&response));

    if response.first().copied() != Some(RESPONSE_SETUP_OK) {
        return Err(log_string_error(
            "setup rejected",
            "Device rejected setup payload",
            "shared",
        ));
    }

    let status = transport
        .query_status()
        .await
        .map_err(|e| log_string_error("post-setup status query failed", e, "shared"))?;

    Ok(build_connection_state(&status))
}

pub async fn do_restart(transport: &Transport) -> Result<(), String> {
    let response = transport
        .send_command(vec![COMMAND_RESTART])
        .await
        .map_err(|e| log_string_error("restart command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_RESTART_OK) {
        return Err(log_string_error(
            "restart rejected",
            "Device rejected restart command",
            "shared",
        ));
    }
    Ok(())
}

pub async fn do_factory_reset(transport: &Transport) -> Result<(), String> {
    let response = transport
        .send_command(vec![COMMAND_FACTORY_RESET])
        .await
        .map_err(|e| log_string_error("factory reset command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_FACTORY_RESET_OK) {
        return Err(log_string_error(
            "factory reset rejected",
            "Device rejected factory reset command",
            "shared",
        ));
    }
    Ok(())
}

pub async fn do_change_password(
    transport: &Transport,
    current_password: &str,
    new_password: &str,
) -> Result<(), String> {
    let mut command = vec![COMMAND_CHANGE_PASSWORD];
    append_field(&mut command, current_password)
        .map_err(|e| log_string_error("change password current encoding failed", e, "shared"))?;
    append_field(&mut command, new_password)
        .map_err(|e| log_string_error("change password new encoding failed", e, "shared"))?;

    let response = transport
        .send_command(command)
        .await
        .map_err(|e| log_string_error("change password command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_CHANGE_PASSWORD_OK) {
        return Err(log_string_error(
            "change password rejected",
            "Device rejected password change",
            "shared",
        ));
    }
    Ok(())
}

pub async fn do_update_wifi(
    transport: &Transport,
    ssid: &str,
    password: &str,
) -> Result<(), String> {
    let mut command = vec![COMMAND_UPDATE_WIFI];
    append_field(&mut command, ssid)
        .map_err(|e| log_string_error("update wifi ssid encoding failed", e, "shared"))?;
    append_field(&mut command, password)
        .map_err(|e| log_string_error("update wifi password encoding failed", e, "shared"))?;

    let response = transport
        .send_command(command)
        .await
        .map_err(|e| log_string_error("update wifi command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_UPDATE_WIFI_OK) {
        return Err(log_string_error(
            "update wifi rejected",
            "Device rejected wifi settings update",
            "shared",
        ));
    }
    Ok(())
}

pub async fn do_get_firmware_info(transport: &Transport) -> Result<FirmwareInfoResult, String> {
    let info = transport
        .query_firmware_info()
        .await
        .map_err(|e| log_string_error("firmware info query failed", e, "shared"))?;
    Ok(FirmwareInfoResult {
        version: info.version,
        hardware_rev: info.hardware_rev,
        uptime_secs: info.uptime_secs,
    })
}

pub async fn do_update_firmware(
    transport: &Transport,
    firmware_data: Vec<u8>,
    app: &AppHandle,
    chunk_size: usize,
    label: &str,
) -> Result<(), String> {
    let total_size = firmware_data.len();
    tracing::info!("[{}] firmware update: {} bytes", label, total_size);

    let _ = app.emit(
        "firmware-update-progress",
        FirmwareUpdateProgress {
            step: "preparing".into(),
            progress: 0.0,
            message: "Preparing firmware update...".into(),
        },
    );

    let mut begin_cmd = vec![COMMAND_OTA_BEGIN];
    begin_cmd.extend_from_slice(&(total_size as u32).to_le_bytes());
    let response = transport
        .send_command(begin_cmd)
        .await
        .map_err(|e| log_string_error("OTA begin failed", e, label))?;
    if response.first().copied() != Some(RESPONSE_OTA_BEGIN_OK) {
        return Err(log_string_error(
            "OTA begin rejected",
            "Device rejected OTA begin command",
            label,
        ));
    }

    let total_chunks = (total_size + chunk_size - 1) / chunk_size;

    for (seq, chunk_start) in (0..total_size).step_by(chunk_size).enumerate() {
        let chunk_end = (chunk_start + chunk_size).min(total_size);
        let chunk = &firmware_data[chunk_start..chunk_end];

        let mut data_cmd = vec![COMMAND_OTA_DATA];
        data_cmd.extend_from_slice(&(seq as u32).to_le_bytes());
        data_cmd.extend_from_slice(chunk);

        let response = transport
            .send_command(data_cmd)
            .await
            .map_err(|e| log_string_error("OTA data failed", e, label))?;
        if response.first().copied() != Some(RESPONSE_OTA_DATA_OK) {
            return Err(log_string_error(
                "OTA data rejected",
                "Device rejected OTA data chunk",
                label,
            ));
        }

        let progress = ((seq + 1) as f32 / total_chunks as f32) * 100.0;
        let _ = app.emit(
            "firmware-update-progress",
            FirmwareUpdateProgress {
                step: "sending".into(),
                progress,
                message: format!("Sending firmware data... ({}/{})", seq + 1, total_chunks),
            },
        );
    }

    let _ = app.emit(
        "firmware-update-progress",
        FirmwareUpdateProgress {
            step: "verifying".into(),
            progress: 100.0,
            message: "Verifying firmware image...".into(),
        },
    );

    let response = transport
        .send_command(vec![COMMAND_OTA_END])
        .await
        .map_err(|e| log_string_error("OTA end failed", e, label))?;
    if response.first().copied() != Some(RESPONSE_OTA_END_OK) {
        return Err(log_string_error(
            "OTA end rejected",
            "Device rejected OTA end command",
            label,
        ));
    }

    let _ = app.emit(
        "firmware-update-progress",
        FirmwareUpdateProgress {
            step: "complete".into(),
            progress: 100.0,
            message: "Firmware update complete. Device is rebooting...".into(),
        },
    );

    Ok(())
}

pub async fn do_download_and_update_firmware(
    transport: &Transport,
    download_url: &str,
    app: &AppHandle,
    chunk_size: usize,
    label: &str,
) -> Result<(), String> {
    let url = download_url.to_string();
    let firmware_data = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let response = ureq::get(&url)
            .call()
            .map_err(|error| format!("Failed to download firmware: {}", error))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Failed to read firmware data: {}", error))?;
        Ok(bytes)
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
    .map_err(|error| log_string_error("firmware download failed", error, label))?;

    tracing::info!("[{}] downloaded firmware: {} bytes", label, firmware_data.len());
    do_update_firmware(transport, firmware_data, app, chunk_size, label).await
}
