use alloc::string::String;

use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};

pub const PASSWORD_SALT_LEN: usize = 16;
pub const PASSWORD_HASH_LEN: usize = 32;
pub const DEFAULT_PBKDF2_ITERATIONS: u32 = 100_000;

const PBKDF2_PREFIX: &str = "pbkdf2$sha256$";
const PBKDF2_SCHEME: &str = "pbkdf2$sha256$";

const LEGACY_HASH_HEX_LEN: usize = 64;

pub fn hash_password(password: &str, salt: &[u8], iterations: u32) -> String {
    let iterations = iterations.max(1);
    let mut derived = [0u8; PASSWORD_HASH_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut derived);

    let mut out = String::new();
    out.push_str(PBKDF2_PREFIX);
    push_u32(&mut out, iterations);
    out.push('$');
    push_hex(&mut out, salt);
    out.push('$');
    push_hex(&mut out, &derived);
    out
}

pub fn verify_password(stored_hash: &str, password: &str) -> bool {
    if let Some(rest) = stored_hash.strip_prefix(PBKDF2_SCHEME) {
        return verify_pbkdf2(rest, password);
    }

    verify_legacy_sha256(stored_hash, password)
}

pub fn is_hashed_password(stored: &str) -> bool {
    stored.starts_with(PBKDF2_PREFIX)
        || (stored.len() == LEGACY_HASH_HEX_LEN && is_lower_hex(stored.as_bytes()))
}

fn verify_pbkdf2(rest: &str, password: &str) -> bool {
    let mut parts = rest.split('$');
    let iterations = parts.next();
    let salt_hex = parts.next();
    let hash_hex = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let (Some(iterations), Some(salt_hex), Some(hash_hex)) = (iterations, salt_hex, hash_hex) else {
        return false;
    };
    let Some(iterations) = parse_u32(iterations) else {
        return false;
    };
    if iterations == 0 {
        return false;
    }

    let Some(salt) = decode_hex::<PASSWORD_SALT_LEN>(salt_hex) else {
        return false;
    };

    let mut stored = [0u8; PASSWORD_HASH_LEN];
    if !decode_hex_into(hash_hex, &mut stored) {
        return false;
    }

    let mut derived = [0u8; PASSWORD_HASH_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut derived);

    constant_time_eq(&stored, &derived)
}

fn verify_legacy_sha256(stored_hash: &str, password: &str) -> bool {
    if stored_hash.len() != LEGACY_HASH_HEX_LEN || !is_lower_hex(stored_hash.as_bytes()) {
        return false;
    }

    let digest = Sha256::digest(password.as_bytes());
    let mut expected = [0u8; LEGACY_HASH_HEX_LEN];
    let _ = write_hex(&mut expected, &digest);

    constant_time_eq(stored_hash.as_bytes(), &expected)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn push_u32(out: &mut String, value: u32) {
    let mut buf = [0u8; 10];
    let mut index = buf.len();
    let mut value = value;
    loop {
        index -= 1;
        buf[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for byte in &buf[index..] {
        out.push(char::from(*byte));
    }
}

fn parse_u32(value: &str) -> Option<u32> {
    if value.is_empty() {
        return None;
    }
    let mut result: u32 = 0;
    for byte in value.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        result = result.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(result)
}

fn push_hex(out: &mut String, bytes: &[u8]) {
    for byte in bytes {
        out.push(hex_digit(byte >> 4));
        out.push(hex_digit(byte & 0x0F));
    }
}

fn write_hex(out: &mut [u8], bytes: &[u8]) -> Option<usize> {
    let needed = bytes.len().checked_mul(2)?;
    if out.len() < needed {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        out[index * 2] = hex_digit(byte >> 4) as u8;
        out[index * 2 + 1] = hex_digit(byte & 0x0F) as u8;
    }
    Some(needed)
}

fn hex_digit(nibble: u8) -> char {
    char::from_digit(u32::from(nibble), 16).unwrap_or('0')
}

fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    let mut out = [0u8; N];
    if decode_hex_into(value, &mut out) {
        Some(out)
    } else {
        None
    }
}

fn decode_hex_into(value: &str, out: &mut [u8]) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != out.len() * 2 {
        return false;
    }
    let mut index = 0;
    while index < out.len() {
        let high = hex_value(bytes[index * 2]);
        let low = hex_value(bytes[index * 2 + 1]);
        match (high, low) {
            (Some(high), Some(low)) => out[index] = (high << 4) | low,
            _ => return false,
        }
        index += 1;
    }
    true
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_lower_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || (0x61..=0x66).contains(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: [u8; PASSWORD_SALT_LEN] = [0x11; PASSWORD_SALT_LEN];

    fn legacy_sha256_hex(password: &str) -> String {
        let digest = Sha256::digest(password.as_bytes());
        let mut out = String::new();
        push_hex(&mut out, &digest);
        out
    }

    #[test]
    fn hash_is_deterministic_and_self_describing() {
        let hash = hash_password("correct horse battery staple", &SALT, 1_000);
        assert!(hash.starts_with(PBKDF2_PREFIX));
        assert_eq!(hash, hash_password("correct horse battery staple", &SALT, 1_000));
        assert_ne!(hash, hash_password("correct horse battery stapl", &SALT, 1_000));
        let other_salt = [0x22; PASSWORD_SALT_LEN];
        assert_ne!(hash, hash_password("correct horse battery staple", &other_salt, 1_000));
    }

    #[test]
    fn verify_accepts_matching_password() {
        let hash = hash_password("hunter22", &SALT, 1_000);
        assert!(verify_password(&hash, "hunter22"));
        assert!(is_hashed_password(&hash));
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let hash = hash_password("hunter22", &SALT, 1_000);
        assert!(!verify_password(&hash, "hunter2"));
        assert!(!verify_password(&hash, ""));
        assert!(!verify_password(&hash, "Hunter22"));
    }

    #[test]
    fn verify_rejects_malformed_stored_values() {
        assert!(!verify_password("", "anything"));
        assert!(!verify_password("not-a-hash", "anything"));
        assert!(!verify_password(&"z".repeat(LEGACY_HASH_HEX_LEN), "anything"));
        assert!(!verify_password(&"AB".repeat(LEGACY_HASH_HEX_LEN / 2), "anything"));
        assert!(!verify_password("pbkdf2$sha256$0$00$00", "anything"));
        assert!(!verify_password("pbkdf2$sha256$abc$00$00", "anything"));
        assert!(!verify_password("pbkdf2$sha512$10$00$00", "anything"));
    }

    #[test]
    fn legacy_sha256_hashes_still_verify() {
        let legacy = legacy_sha256_hex("hunter22");
        assert_eq!(legacy.len(), LEGACY_HASH_HEX_LEN);
        assert!(verify_password(&legacy, "hunter22"));
        assert!(!verify_password(&legacy, "hunter2"));
        assert!(is_hashed_password(&legacy));
    }

    #[test]
    fn empty_password_hashes_like_any_other_value() {
        let hash = hash_password("", &SALT, 1_000);
        assert!(verify_password(&hash, ""));
        assert!(!verify_password(&hash, "x"));
        assert!(is_hashed_password(&hash));
    }

    #[test]
    fn is_hashed_password_requires_a_recognised_format() {
        assert!(is_hashed_password(&"a1".repeat(32)));
        assert!(!is_hashed_password("a1".repeat(31).as_str()));
        assert!(!is_hashed_password("a1".repeat(33).as_str()));
        assert!(!is_hashed_password(""));
        assert!(!is_hashed_password(&"g".repeat(64)));
        assert!(!is_hashed_password("plaintext password"));
    }
}
