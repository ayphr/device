use esp_idf_svc::sntp::{EspSntp, SntpConf};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Timestamps before this are treated as "not synced yet". This is 2020-09-13.
const MIN_VALID_UNIX_MS: u64 = 1_600_000_000_000;

static SYNCED: AtomicBool = AtomicBool::new(false);

/// Starts SNTP in the background. The returned handle must be kept alive for
/// as long as the device is running; dropping it stops the SNTP service.
pub fn start() -> Option<EspSntp<'static>> {
    let conf: SntpConf<'static> = SntpConf::default();

    match EspSntp::new_with_callback(&conf, |_| {
        SYNCED.store(true, Ordering::SeqCst);
        info!("System clock synchronized via SNTP");
    }) {
        Ok(sntp) => {
            info!("SNTP started");
            Some(sntp)
        }
        Err(error) => {
            warn!("Failed to start SNTP: {:?}", error);
            None
        }
    }
}

pub fn is_synced() -> bool {
    SYNCED.load(Ordering::SeqCst) && now_unix_ms().is_some()
}

pub fn now_unix_ms() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
        .filter(|ms| *ms >= MIN_VALID_UNIX_MS)
}
