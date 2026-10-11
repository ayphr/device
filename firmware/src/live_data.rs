use std::sync::Mutex;

use ayphr_protocol::SensorPacket;

static LATEST: Mutex<Option<SensorPacket>> = Mutex::new(None);

/// Stores the most recent sensor reading for command handlers to serve.
pub fn update(packet: SensorPacket) {
    match LATEST.lock() {
        Ok(mut guard) => *guard = Some(packet),
        Err(poisoned) => *poisoned.into_inner() = Some(packet),
    }
}

/// Returns the most recent sensor reading, if one has been sampled yet.
pub fn latest() -> Option<SensorPacket> {
    match LATEST.lock() {
        Ok(guard) => *guard,
        Err(poisoned) => *poisoned.into_inner(),
    }
}
