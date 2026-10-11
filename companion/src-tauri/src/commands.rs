use ayphr_protocol::{
    append_field, verify_firmware_digest_signature, verify_firmware_signature, FirmwareHasher,
    COMMAND_APPLY_SETUP, COMMAND_CHANGE_PASSWORD, COMMAND_FACTORY_RESET, COMMAND_OTA_BEGIN,
    COMMAND_OTA_DATA, COMMAND_OTA_END, COMMAND_OTA_ROLLBACK, COMMAND_RENAME_DEVICE,
    COMMAND_RESTART, COMMAND_UPDATE_WIFI, FIRMWARE_SIGNATURE_LEN, MAX_DEVICE_NAME_LEN,
    RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_FACTORY_RESET_OK, RESPONSE_OTA_BEGIN_OK,
    RESPONSE_OTA_DATA_OK, RESPONSE_OTA_END_OK, RESPONSE_OTA_ROLLBACK_OK, RESPONSE_RENAME_OK,
    RESPONSE_RESTART_OK, RESPONSE_SETUP_OK, RESPONSE_UPDATE_WIFI_OK,
};
use tauri::{AppHandle, Emitter};

use crate::protocol::{format_bytes, log_string_error};
use crate::transport::Transport;
use crate::types::{
    BleConnectionState, FirmwareInfoResult, FirmwareUpdateProgress, LiveDataResult, ParsedStatus,
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

pub async fn do_rename_device(transport: &Transport, name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Device name cannot be empty".to_string());
    }
    if trimmed.len() > MAX_DEVICE_NAME_LEN {
        return Err(format!(
            "Device name must be at most {} bytes",
            MAX_DEVICE_NAME_LEN
        ));
    }

    let mut command = vec![COMMAND_RENAME_DEVICE];
    append_field(&mut command, trimmed)
        .map_err(|e| log_string_error("rename encoding failed", e, "shared"))?;

    let response = transport
        .send_command(command)
        .await
        .map_err(|e| log_string_error("rename command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_RENAME_OK) {
        return Err(log_string_error(
            "rename rejected",
            "Device rejected rename command",
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

pub async fn do_get_live_data(transport: &Transport) -> Result<Option<LiveDataResult>, String> {
    let packet = transport
        .query_live_data()
        .await
        .map_err(|e| log_string_error("live data query failed", e, "shared"))?;

    Ok(packet.map(|packet| LiveDataResult {
        packet_type: packet.packet_type,
        temperature: packet.temperature,
        humidity: packet.humidity,
        air_pressure: packet.air_pressure,
        serial: packet.serial,
    }))
}

pub fn decode_signature_hex(value: &str) -> Result<[u8; FIRMWARE_SIGNATURE_LEN], String> {
    let trimmed = value.trim();
    if trimmed.len() != FIRMWARE_SIGNATURE_LEN * 2 {
        return Err(format!(
            "Firmware signature must be {} hex characters, got {}",
            FIRMWARE_SIGNATURE_LEN * 2,
            trimmed.len()
        ));
    }
    let mut out = [0u8; FIRMWARE_SIGNATURE_LEN];
    for (index, byte) in out.iter_mut().enumerate() {
        let high = hex_nibble(trimmed.as_bytes()[index * 2])?;
        let low = hex_nibble(trimmed.as_bytes()[index * 2 + 1])?;
        *byte = (high << 4) | low;
    }
    Ok(out)
}

pub fn load_signature_for_firmware(
    firmware_path: &str,
) -> Result<[u8; FIRMWARE_SIGNATURE_LEN], String> {
    let signature_path = format!("{firmware_path}.sig");
    let bytes = std::fs::read(&signature_path).map_err(|error| {
        format!(
            "Missing firmware signature file '{signature_path}': {error}. Install a signed release or provide the matching .sig file."
        )
    })?;

    let trimmed: Vec<u8> = bytes
        .into_iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();

    if trimmed.len() == FIRMWARE_SIGNATURE_LEN {
        let mut out = [0u8; FIRMWARE_SIGNATURE_LEN];
        out.copy_from_slice(&trimmed);
        return Ok(out);
    }

    if trimmed.len() == FIRMWARE_SIGNATURE_LEN * 2 {
        let text = core::str::from_utf8(&trimmed)
            .map_err(|_| "Firmware signature file is not valid hex".to_string())?;
        return decode_signature_hex(text);
    }

    Err(format!(
        "Firmware signature file '{signature_path}' must contain {} raw bytes or {} hex characters",
        FIRMWARE_SIGNATURE_LEN,
        FIRMWARE_SIGNATURE_LEN * 2
    ))
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!(
            "Invalid hex character '{}' in firmware signature",
            byte as char
        )),
    }
}

struct OtaWriter<'a> {
    transport: &'a Transport,
    app: &'a AppHandle,
    label: &'a str,
    chunk_size: usize,
    window: usize,
    total_size: usize,
    total_chunks: usize,
    seq: u32,
    pending: Vec<Vec<u8>>,
    buffered: Vec<u8>,
}

impl<'a> OtaWriter<'a> {
    fn new(
        transport: &'a Transport,
        app: &'a AppHandle,
        label: &'a str,
        chunk_size: usize,
        total_size: usize,
    ) -> Self {
        let chunk_size = chunk_size.max(1);
        Self {
            transport,
            app,
            label,
            chunk_size,
            window: transport.ota_window().max(1),
            total_size,
            total_chunks: total_size.div_ceil(chunk_size),
            seq: 0,
            pending: Vec::new(),
            buffered: Vec::with_capacity(chunk_size),
        }
    }

    async fn begin(&mut self, signature: &[u8; FIRMWARE_SIGNATURE_LEN]) -> Result<(), String> {
        let mut begin_cmd = vec![COMMAND_OTA_BEGIN];
        begin_cmd.extend_from_slice(&(self.total_size as u32).to_le_bytes());
        begin_cmd.extend_from_slice(signature);
        let response = self
            .transport
            .send_command(begin_cmd)
            .await
            .map_err(|e| log_string_error("OTA begin failed", e, self.label))?;
        if response.first().copied() != Some(RESPONSE_OTA_BEGIN_OK) {
            return Err(log_string_error(
                "OTA begin rejected",
                "Device rejected OTA begin command",
                self.label,
            ));
        }
        Ok(())
    }

    async fn feed(&mut self, mut data: &[u8]) -> Result<(), String> {
        while !data.is_empty() {
            let take = (self.chunk_size - self.buffered.len()).min(data.len());
            self.buffered.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buffered.len() == self.chunk_size {
                self.enqueue_chunk().await?;
            }
        }
        Ok(())
    }

    async fn enqueue_chunk(&mut self) -> Result<(), String> {
        let payload = std::mem::take(&mut self.buffered);
        self.buffered = Vec::with_capacity(self.chunk_size);

        let mut command = Vec::with_capacity(1 + 4 + payload.len());
        command.push(COMMAND_OTA_DATA);
        command.extend_from_slice(&self.seq.to_le_bytes());
        command.extend_from_slice(&payload);
        self.pending.push(command);
        self.seq += 1;

        if self.pending.len() >= self.window {
            self.flush().await?;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }

        let batch = std::mem::take(&mut self.pending);
        let responses = self
            .transport
            .send_commands_batch(batch)
            .await
            .map_err(|e| log_string_error("OTA data failed", e, self.label))?;
        for response in &responses {
            if response.first().copied() != Some(RESPONSE_OTA_DATA_OK) {
                return Err(log_string_error(
                    "OTA data rejected",
                    "Device rejected OTA data chunk",
                    self.label,
                ));
            }
        }

        let progress = (self.seq as f32 / self.total_chunks.max(1) as f32) * 100.0;
        let _ = self.app.emit(
            "firmware-update-progress",
            FirmwareUpdateProgress {
                step: "sending".into(),
                progress,
                message: format!(
                    "Sending firmware data... ({}/{})",
                    self.seq, self.total_chunks
                ),
            },
        );
        Ok(())
    }

    async fn finish(&mut self) -> Result<(), String> {
        if !self.buffered.is_empty() {
            self.enqueue_chunk().await?;
        }
        self.flush().await?;

        let _ = self.app.emit(
            "firmware-update-progress",
            FirmwareUpdateProgress {
                step: "verifying".into(),
                progress: 100.0,
                message: "Verifying firmware image...".into(),
            },
        );

        let response = self
            .transport
            .send_command(vec![COMMAND_OTA_END])
            .await
            .map_err(|e| log_string_error("OTA end failed", e, self.label))?;
        if response.first().copied() != Some(RESPONSE_OTA_END_OK) {
            return Err(log_string_error(
                "OTA end rejected",
                "Device rejected OTA end command",
                self.label,
            ));
        }

        let _ = self.app.emit(
            "firmware-update-progress",
            FirmwareUpdateProgress {
                step: "complete".into(),
                progress: 100.0,
                message: "Firmware update complete. Device is rebooting...".into(),
            },
        );
        Ok(())
    }
}

fn emit_preparing(app: &AppHandle) {
    let _ = app.emit(
        "firmware-update-progress",
        FirmwareUpdateProgress {
            step: "preparing".into(),
            progress: 0.0,
            message: "Preparing firmware update...".into(),
        },
    );
}

pub async fn do_update_firmware(
    transport: &Transport,
    firmware_data: Vec<u8>,
    signature: [u8; FIRMWARE_SIGNATURE_LEN],
    app: &AppHandle,
    chunk_size: usize,
    label: &str,
) -> Result<(), String> {
    let total_size = firmware_data.len();
    tracing::info!("[{}] firmware update: {} bytes", label, total_size);

    if !verify_firmware_signature(&firmware_data, &signature) {
        return Err(log_string_error(
            "firmware signature invalid",
            "Firmware signature verification failed; refusing to flash",
            label,
        ));
    }

    emit_preparing(app);

    let mut writer = OtaWriter::new(transport, app, label, chunk_size, total_size);
    writer.begin(&signature).await?;
    writer.feed(&firmware_data).await?;
    writer.finish().await
}

enum FirmwareStream {
    /// Total image size from the `Content-Length` header.
    Size(usize),
    /// The next slice of image bytes.
    Chunk(Vec<u8>),
    /// The stream ended; carries the SHA-256 digest of everything received.
    Done([u8; 32]),
}

fn hex_from_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stream_firmware_download(
    url: &str,
    chunk_size: usize,
    tx: &tokio::sync::mpsc::Sender<Result<FirmwareStream, String>>,
) -> Result<(), String> {
    use std::io::Read;

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            // The stream needs to know the exact byte count up front and must
            // hash the exact bytes written to the device, so request an
            // unencoded body and rely on Content-Length rather than ureq's
            // transparent decompression (which hides the length).
            .accept_encoding("identity")
            .timeout_global(Some(std::time::Duration::from_secs(300)))
            .build(),
    );
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("Failed to download firmware: {error}"))?;

    let body = response.into_body();
    let total_size = body
        .content_length()
        .ok_or_else(|| "Firmware response is missing a Content-Length header".to_string())?
        as usize;
    if total_size == 0 {
        return Err("Firmware download was empty".to_string());
    }

    tx.blocking_send(Ok(FirmwareStream::Size(total_size)))
        .map_err(|_| "Firmware download was canceled".to_string())?;

    let mut reader = body.into_reader();
    let mut hasher = FirmwareHasher::new();
    let mut buffer = vec![0u8; chunk_size.max(16 * 1024)];
    let mut received = 0usize;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Failed to read firmware data: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        received += read;
        tx.blocking_send(Ok(FirmwareStream::Chunk(buffer[..read].to_vec())))
            .map_err(|_| "Firmware download was canceled".to_string())?;
    }

    if received != total_size {
        return Err(format!(
            "Firmware download was truncated: expected {total_size} bytes, received {received}"
        ));
    }

    tx.blocking_send(Ok(FirmwareStream::Done(hasher.finalize())))
        .map_err(|_| "Firmware download was canceled".to_string())?;
    Ok(())
}

pub async fn do_download_and_update_firmware(
    transport: &Transport,
    download_url: &str,
    expected_sha256: &str,
    expected_signature: &str,
    app: &AppHandle,
    chunk_size: usize,
    label: &str,
) -> Result<(), String> {
    let signature = decode_signature_hex(expected_signature)
        .map_err(|error| log_string_error("firmware signature invalid", error, label))?;
    let expected_sha256 = expected_sha256.to_string();
    let url = download_url.to_string();
    let chunk_size = chunk_size.max(1);

    // The download runs on a blocking thread and hands chunks to the async side
    // as they arrive, so peak memory stays bounded by the channel plus one OTA
    // window rather than the whole image.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<FirmwareStream, String>>(8);
    tokio::task::spawn_blocking(move || {
        if let Err(error) = stream_firmware_download(&url, chunk_size, &tx) {
            let _ = tx.blocking_send(Err(error));
        }
    });

    let total_size = match rx.recv().await {
        Some(Ok(FirmwareStream::Size(size))) => size,
        Some(Ok(_)) => {
            return Err(log_string_error(
                "firmware download failed",
                "Unexpected response from the firmware download",
                label,
            ))
        }
        Some(Err(error)) => return Err(log_string_error("firmware download failed", error, label)),
        None => {
            return Err(log_string_error(
                "firmware download failed",
                "Firmware download ended before any data arrived",
                label,
            ))
        }
    };

    tracing::info!("[{}] streaming firmware: {} bytes", label, total_size);
    emit_preparing(app);

    let mut writer = OtaWriter::new(transport, app, label, chunk_size, total_size);
    writer.begin(&signature).await?;

    let mut digest = None;
    while let Some(message) = rx.recv().await {
        match message {
            Ok(FirmwareStream::Chunk(data)) => writer.feed(&data).await?,
            Ok(FirmwareStream::Done(streamed_digest)) => {
                digest = Some(streamed_digest);
                break;
            }
            // A second size message cannot occur; ignore defensively.
            Ok(FirmwareStream::Size(_)) => {}
            Err(error) => return Err(log_string_error("firmware download failed", error, label)),
        }
    }

    let digest = digest.ok_or_else(|| {
        log_string_error(
            "firmware download failed",
            "Firmware download ended without a completion marker",
            label,
        )
    })?;

    let actual_sha256 = hex_from_digest(&digest);
    if expected_sha256.is_empty() || !actual_sha256.eq_ignore_ascii_case(&expected_sha256) {
        return Err(log_string_error(
            "firmware checksum mismatch",
            format!("expected sha256 {}, got {}", expected_sha256, actual_sha256),
            label,
        ));
    }
    if !verify_firmware_digest_signature(&digest, &signature) {
        return Err(log_string_error(
            "firmware signature invalid",
            "Firmware signature verification failed; refusing to flash",
            label,
        ));
    }

    writer.finish().await
}

pub async fn do_ota_rollback(transport: &Transport) -> Result<(), String> {
    let response = transport
        .send_command(vec![COMMAND_OTA_ROLLBACK])
        .await
        .map_err(|e| log_string_error("OTA rollback command failed", e, "shared"))?;

    if response.first().copied() != Some(RESPONSE_OTA_ROLLBACK_OK) {
        return Err(log_string_error(
            "OTA rollback rejected",
            "Device rejected rollback command",
            "shared",
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_firmware_release_metadata(
) -> Result<crate::types::FirmwareReleaseMetadata, String> {
    tokio::task::spawn_blocking(|| {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(std::time::Duration::from_secs(10)))
                .build(),
        );
        let response = agent
            .get("https://github.com/ayphr/device/releases/download/firmware-latest/version.json")
            .call()
            .map_err(|error| format!("Failed to fetch firmware release metadata: {error}"))?;
        let bytes = response
            .into_body()
            .read_to_vec()
            .map_err(|error| format!("Failed to read release metadata: {error}"))?;
        let metadata: crate::types::FirmwareReleaseMetadata = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Failed to parse release metadata: {error}"))?;
        Ok(metadata)
    })
    .await
    .map_err(|error| format!("Task join error: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature_hex(byte: u8) -> String {
        format!("{byte:02x}").repeat(FIRMWARE_SIGNATURE_LEN)
    }

    #[test]
    fn decode_signature_hex_accepts_upper_and_lower_case() {
        let lower = signature_hex(0xab);
        let upper = lower.to_uppercase();

        let from_lower = decode_signature_hex(&lower).unwrap();
        let from_upper = decode_signature_hex(&upper).unwrap();

        assert_eq!(from_lower, from_upper);
        assert_eq!(from_lower[0], 0xab);
        assert_eq!(from_lower[FIRMWARE_SIGNATURE_LEN - 1], 0xab);
    }

    #[test]
    fn decode_signature_hex_trims_surrounding_whitespace() {
        let padded = format!("  {}\n", signature_hex(0x01));
        assert_eq!(decode_signature_hex(&padded).unwrap()[0], 0x01);
    }

    #[test]
    fn decode_signature_hex_rejects_bad_input() {
        assert!(decode_signature_hex("").is_err());
        assert!(decode_signature_hex(&"ab".repeat(FIRMWARE_SIGNATURE_LEN - 1)).is_err());
        assert!(decode_signature_hex(&"ab".repeat(FIRMWARE_SIGNATURE_LEN + 1)).is_err());
        assert!(decode_signature_hex(&"zz".repeat(FIRMWARE_SIGNATURE_LEN)).is_err());
    }

    fn with_temp_signature_file(name: &str, contents: &[u8], test: impl FnOnce(&str)) {
        let dir =
            std::env::temp_dir().join(format!("ayphr-sig-test-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        let firmware_path = dir.join("firmware.bin");
        std::fs::write(&firmware_path, b"firmware").unwrap();
        std::fs::write(dir.join("firmware.bin.sig"), contents).unwrap();

        test(firmware_path.to_str().unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_signature_reads_hex_file() {
        let hex = signature_hex(0x5a);
        with_temp_signature_file("hex", format!("{hex}\n").as_bytes(), |path| {
            let signature = load_signature_for_firmware(path).unwrap();
            assert_eq!(signature, [0x5a; FIRMWARE_SIGNATURE_LEN]);
        });
    }

    #[test]
    fn load_signature_reads_raw_bytes_file() {
        let raw = [0x42u8; FIRMWARE_SIGNATURE_LEN];
        with_temp_signature_file("raw", &raw, |path| {
            let signature = load_signature_for_firmware(path).unwrap();
            assert_eq!(signature, raw);
        });
    }

    #[test]
    fn load_signature_rejects_bad_lengths() {
        with_temp_signature_file("short", b"not a signature", |path| {
            assert!(load_signature_for_firmware(path).is_err());
        });
    }

    #[test]
    fn load_signature_errors_when_file_missing() {
        let missing = std::env::temp_dir().join("ayphr-does-not-exist/firmware.bin");
        assert!(load_signature_for_firmware(missing.to_str().unwrap()).is_err());
    }
}
