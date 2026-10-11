use std::sync::OnceLock;

use esp_idf_svc::sys::esp_efuse_mac_get_default;
use log::warn;

static SERIAL: OnceLock<u32> = OnceLock::new();

/// Returns a stable 32-bit serial derived from the chip's factory-programmed
/// base MAC address. The MAC lives in eFuse, so the serial is permanent across
/// firmware flashes and factory resets.
pub fn serial() -> u32 {
    *SERIAL.get_or_init(|| {
        let mut mac = [0u8; 6];
        // Safety: `mac` is a valid pointer to a correctly sized output buffer.
        let result = unsafe { esp_efuse_mac_get_default(mac.as_mut_ptr()) };
        if result != 0 {
            warn!(
                "Failed to read base MAC (esp_err {}); serial falls back to zero",
                result
            );
            return 0;
        }
        derive_serial(&mac)
    })
}

/// Folds the 48-bit MAC into a 32-bit value using FNV-1a.
fn derive_serial(mac: &[u8; 6]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in mac {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}
