use esp_idf_svc::ota::{EspOta, EspOtaUpdate};
use log::info;
use esp_idf_svc::io::Write;
use std::sync::Mutex;

pub struct OtaSession {
    update: Option<EspOtaUpdate<'static>>,
    bytes_written: usize,
    total_size: usize,
}

static OTA_STATE: Mutex<OtaSession> = Mutex::new(OtaSession {
    update: None,
    bytes_written: 0,
    total_size: 0,
});

pub fn begin(total_size: usize) -> Result<(), &'static str> {
    let mut state = OTA_STATE.lock().map_err(|_| "OTA state lock poisoned")?;
    
    let ota = Box::leak(Box::new(
        EspOta::new().map_err(|_| "Failed to create OTA handle")?,
    ));

    let update = ota
        .initiate_update()
        .map_err(|_| "Failed to initiate OTA update")?;

    state.update = Some(update);
    state.bytes_written = 0;
    state.total_size = total_size;
    info!("OTA update started, total_size={}", total_size);
    Ok(())
}

pub fn write_data(data: &[u8]) -> Result<(), &'static str> {
    let mut state = OTA_STATE.lock().map_err(|_| "OTA state lock poisoned")?;
    let update = state
        .update
        .as_mut()
        .ok_or("OTA not initialized; send BEGIN first")?;

    update
        .write_all(data)
        .map_err(|_| "Failed to write OTA data to flash")?;

    state.bytes_written += data.len();
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
    let mut state = OTA_STATE.lock().map_err(|_| "OTA state lock poisoned")?;
    let update = state
        .update
        .take()
        .ok_or("OTA not initialized; send BEGIN first")?;

    update
        .complete()
        .map_err(|_| "OTA image validation failed")?;

    info!(
        "OTA update complete ({} bytes written), rebooting...",
        state.bytes_written
    );
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        esp_idf_svc::hal::reset::restart();
    });
    Ok(())
}
