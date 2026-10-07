use alloc::string::String;

use sha2::{Digest, Sha256};

/// Length in characters of a hex-encoded SHA-256 digest.
const HASH_HEX_LEN: usize = 64;

/// Returns the hex-encoded SHA-256 digest of `password`.
pub fn hash_password(password: &str) -> String {
    let digest = Sha256::digest(password.as_bytes());
    let mut out = String::with_capacity(HASH_HEX_LEN);
    for byte in digest {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((byte & 0x0F) as u32, 16).unwrap_or('0'));
    }
    out
}

/// Verifies `password` against a stored hex digest. Comparison is
/// constant-time with respect to the digest bytes so timing cannot be used to
/// recover the stored hash byte by byte.
pub fn verify_password(stored_hash: &str, password: &str) -> bool {
    if stored_hash.len() != HASH_HEX_LEN || !is_hex(stored_hash.as_bytes()) {
        return false;
    }

    let candidate = hash_password(password);
    let stored = stored_hash.as_bytes();
    let candidate = candidate.as_bytes();

    let mut diff = 0u8;
    for index in 0..HASH_HEX_LEN {
        diff |= stored[index] ^ candidate[index];
    }
    diff == 0
}

/// Whether a stored credential already looks like a hex SHA-256 digest (i.e.
/// it has been hashed and can be compared directly against a fresh hash).
pub fn is_hashed_password(stored: &str) -> bool {
    stored.len() == HASH_HEX_LEN && is_hex(stored.as_bytes())
}

fn is_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || (0x61..=0x66).contains(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_hex_encoded() {
        let hash = hash_password("correct horse battery staple");
        assert_eq!(hash.len(), HASH_HEX_LEN);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash, hash_password("correct horse battery staple"));
        assert_ne!(hash, hash_password("correct horse battery stapl"));
    }

    #[test]
    fn verify_accepts_matching_password() {
        let hash = hash_password("hunter22");
        assert!(verify_password(&hash, "hunter22"));
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let hash = hash_password("hunter22");
        assert!(!verify_password(&hash, "hunter2"));
        assert!(!verify_password(&hash, ""));
        assert!(!verify_password(&hash, "Hunter22"));
    }

    #[test]
    fn verify_rejects_malformed_stored_values() {
        assert!(!verify_password("", "anything"));
        assert!(!verify_password("not-a-hash", "anything"));
        assert!(!verify_password(&"z".repeat(HASH_HEX_LEN), "anything"));
        assert!(!verify_password(&"AB".repeat(HASH_HEX_LEN / 2), "anything"));
    }

    #[test]
    fn empty_password_hashes_like_any_other_value() {
        let hash = hash_password("");
        assert!(verify_password(&hash, ""));
        assert!(is_hashed_password(&hash));
    }

    #[test]
    fn is_hashed_password_requires_64_hex_chars() {
        assert!(is_hashed_password(&"a1".repeat(32)));
        assert!(!is_hashed_password("a1".repeat(31).as_str()));
        assert!(!is_hashed_password("a1".repeat(33).as_str()));
        assert!(!is_hashed_password(""));
        assert!(!is_hashed_password(&"g".repeat(64)));
        assert!(!is_hashed_password("plaintext password"));
    }
}
