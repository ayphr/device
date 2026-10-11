use esp_idf_svc::io::Write;
use esp_idf_svc::ota::{EspOta, EspOtaUpdate};
use log::{info, warn};
use std::sync::Mutex;

use ayphr_protocol::{verify_firmware_digest_signature, FirmwareHasher, FIRMWARE_SIGNATURE_LEN};

pub struct OtaSession {
    ota: Option<*mut EspOta>,
    update: Option<EspOtaUpdate<'static>>,
    bytes_written: usize,
    total_size: usize,
    signature: [u8; FIRMWARE_SIGNATURE_LEN],
    hasher: Option<FirmwareHasher>,
    next_seq: u32,
}

unsafe impl Send for OtaSession {}

static OTA_STATE: Mutex<OtaSession> = Mutex::new(OtaSession {
    ota: None,
    update: None,
    bytes_written: 0,
    total_size: 0,
    signature: [0u8; FIRMWARE_SIGNATURE_LEN],
    hasher: None,
    next_seq: 0,
});

const MAX_OTA_PARTITION_SIZE: usize = 0x1E0000;

fn lock_ota_state() -> Result<std::sync::MutexGuard<'static, OtaSession>, &'static str> {
    match OTA_STATE.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            warn!("OTA mutex recovered from poisoning; resetting session state");
            let mut guard = poisoned.into_inner();
            guard.update = None;
            guard.bytes_written = 0;
            guard.total_size = 0;
            guard.hasher = None;
            guard.next_seq = 0;
            Ok(guard)
        }
    }
}

pub fn begin(
    total_size: usize,
    signature: [u8; FIRMWARE_SIGNATURE_LEN],
) -> Result<(), &'static str> {
    if total_size == 0 {
        return Err("OTA firmware size must be greater than zero");
    }
    if total_size > MAX_OTA_PARTITION_SIZE {
        return Err("OTA firmware size exceeds available flash partition");
    }

    let mut state = lock_ota_state()?;

    if let Some(previous) = state.update.take() {
        drop(previous);
    }

    if state.ota.is_none() {
        let ota = Box::leak(Box::new(
            EspOta::new().map_err(|_| "Failed to create OTA handle")?,
        ));
        state.ota = Some(ota as *mut EspOta);
    }

    let ota_ptr = state.ota.ok_or("OTA handle unavailable")?;
    let update = unsafe { &mut *ota_ptr }
        .initiate_update_with_known_size(total_size)
        .map_err(|_| "Failed to initiate OTA update")?;

    state.update = Some(update);
    state.bytes_written = 0;
    state.total_size = total_size;
    state.signature = signature;
    state.hasher = Some(FirmwareHasher::new());
    state.next_seq = 0;
    info!("OTA update started, total_size={}", total_size);
    Ok(())
}

pub fn write_data(seq: u32, data: &[u8]) -> Result<(), &'static str> {
    let mut state = lock_ota_state()?;

    if state.update.is_none() {
        return Err("OTA not initialized; send BEGIN first");
    }

    if data.is_empty() {
        return Err("OTA data chunk is empty");
    }

    if seq != state.next_seq {
        warn!(
            "Rejecting OTA chunk with sequence {}; expected {}",
            seq, state.next_seq
        );
        return Err("OTA data chunk out of order");
    }

    let remaining = state.total_size.saturating_sub(state.bytes_written);
    if data.len() > remaining {
        return Err("OTA data exceeds declared firmware size");
    }

    if let Some(hasher) = state.hasher.as_mut() {
        hasher.update(data);
    }

    let update = state
        .update
        .as_mut()
        .ok_or("OTA not initialized; send BEGIN first")?;

    update
        .write_all(data)
        .map_err(|_| "Failed to write OTA data to flash")?;

    state.bytes_written += data.len();
    state.next_seq = state.next_seq.wrapping_add(1);
    info!(
        "OTA progress: {}/{} bytes ({:.1}%)",
        state.bytes_written,
        state.total_size,
        if state.total_size > 0 {
            state.bytes_written as f32 / state.total_size as f32 * 100.0
        } else {
            0.0
        }
    );
    Ok(())
}

pub fn end() -> Result<(), &'static str> {
    let mut state = lock_ota_state()?;

    if state.update.is_none() {
        return Err("OTA not initialized; send BEGIN first");
    }

    if state.bytes_written < state.total_size {
        state.update = None;
        state.hasher = None;
        return Err("OTA incomplete: fewer bytes received than declared size");
    }

    let digest = state
        .hasher
        .take()
        .ok_or("OTA session has no active hasher")?
        .finalize();
    if !verify_firmware_digest_signature(&digest, &state.signature) {
        state.update = None;
        warn!("OTA image signature verification failed; aborting update");
        return Err("OTA image signature verification failed");
    }

    let update = state
        .update
        .take()
        .ok_or("OTA not initialized; send BEGIN first")?;

    update
        .complete()
        .map_err(|_| "OTA image validation failed")?;

    info!(
        "OTA update complete ({} bytes written, signature verified), rebooting...",
        state.bytes_written
    );
    crate::command_processor::schedule_restart();
    Ok(())
}

pub fn mark_valid() {
    unsafe {
        esp_idf_svc::sys::esp_ota_mark_app_valid_cancel_rollback();
    }
    info!("OTA app marked valid, rollback cancelled");
}

pub fn rollback() -> Result<(), &'static str> {
    info!("OTA rollback requested, rebooting into previous firmware...");
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        unsafe {
            esp_idf_svc::sys::esp_ota_mark_app_invalid_rollback_and_reboot();
        }
    });
    Ok(())
}
