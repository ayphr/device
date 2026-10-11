use ed25519_compact::{PublicKey, Signature};
use sha2::{Digest, Sha256};

pub const FIRMWARE_SIGNATURE_LEN: usize = 64;
pub const FIRMWARE_PUBLIC_KEY_LEN: usize = 32;

pub const FIRMWARE_PUBLIC_KEY: [u8; FIRMWARE_PUBLIC_KEY_LEN] = [
    0xb8, 0x83, 0x22, 0xaa, 0xbd, 0xab, 0xab, 0xfc, 0xf2, 0x84, 0xf9, 0xe2, 0xa2, 0xc7, 0x57, 0x48,
    0x86, 0x8c, 0xce, 0x24, 0x18, 0xd3, 0xe0, 0x10, 0xe7, 0xc2, 0x7c, 0x3a, 0x60, 0xe8, 0x67, 0x1b,
];

pub fn firmware_sha256(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn verify_firmware_signature(data: &[u8], signature: &[u8]) -> bool {
    verify_firmware_digest_signature(&firmware_sha256(data), signature)
}

pub fn verify_firmware_digest_signature(digest: &[u8; 32], signature: &[u8]) -> bool {
    verify_digest_signature(&FIRMWARE_PUBLIC_KEY, digest, signature)
}

pub fn verify_digest_signature(
    public_key: &[u8; FIRMWARE_PUBLIC_KEY_LEN],
    digest: &[u8; 32],
    signature: &[u8],
) -> bool {
    if signature.len() != FIRMWARE_SIGNATURE_LEN {
        return false;
    }
    let Ok(public_key) = PublicKey::from_slice(public_key) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(signature) else {
        return false;
    };
    public_key.verify(digest, &signature).is_ok()
}

#[derive(Clone, Default)]
pub struct FirmwareHasher {
    inner: Sha256,
}

impl FirmwareHasher {
    pub fn new() -> Self {
        Self {
            inner: Sha256::new(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    pub fn finalize(self) -> [u8; 32] {
        let digest = self.inner.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"ayphr firmware test image";
    const TEST_PUBLIC_KEY: [u8; 32] = [
        0x12, 0xc1, 0x37, 0x6a, 0xfb, 0xdc, 0xa8, 0x1a, 0x4a, 0x1a, 0xd6, 0xc2, 0x08, 0x97, 0x7a,
        0x21, 0x4d, 0xb6, 0x97, 0xa2, 0x7e, 0x0f, 0x89, 0x7b, 0x62, 0x3a, 0x2e, 0xae, 0x8b, 0xa3,
        0x6d, 0xf9,
    ];
    const TEST_DIGEST: [u8; 32] = [
        0xcc, 0x28, 0xed, 0x60, 0x62, 0xb0, 0x9d, 0x77, 0xf4, 0x96, 0x09, 0x93, 0x06, 0xd4, 0x35,
        0x63, 0xd0, 0x1c, 0x13, 0x65, 0xfb, 0xbf, 0x58, 0xc4, 0x09, 0x73, 0x2e, 0x24, 0xb7, 0x58,
        0x10, 0x48,
    ];
    const TEST_SIGNATURE: [u8; 64] = [
        0x68, 0xaf, 0xad, 0xf1, 0x3c, 0x65, 0xaa, 0x0a, 0x39, 0x25, 0xfc, 0x9e, 0x86, 0x7f, 0x71,
        0x8f, 0x7c, 0x19, 0x23, 0x43, 0x54, 0x07, 0x4c, 0x49, 0x08, 0x66, 0x67, 0xdb, 0x49, 0x5f,
        0x4a, 0xee, 0xd2, 0x96, 0x57, 0x5d, 0x18, 0x3c, 0x8c, 0xfb, 0x88, 0x80, 0x6f, 0xe4, 0x94,
        0x53, 0x83, 0x6c, 0xba, 0xa2, 0x4f, 0x02, 0x4e, 0xc6, 0x81, 0x7b, 0x1e, 0x26, 0xa5, 0x20,
        0xaa, 0x93, 0x05, 0x03,
    ];

    #[test]
    fn verifies_a_valid_signature_over_the_digest() {
        assert_eq!(firmware_sha256(TEST_DATA), TEST_DIGEST);
        assert!(verify_digest_signature(
            &TEST_PUBLIC_KEY,
            &TEST_DIGEST,
            &TEST_SIGNATURE
        ));
    }

    #[test]
    fn rejects_tampered_digest_and_signature() {
        let mut wrong_digest = TEST_DIGEST;
        wrong_digest[0] ^= 0x01;
        assert!(!verify_digest_signature(
            &TEST_PUBLIC_KEY,
            &wrong_digest,
            &TEST_SIGNATURE
        ));

        let mut wrong_signature = TEST_SIGNATURE;
        wrong_signature[0] ^= 0x01;
        assert!(!verify_digest_signature(
            &TEST_PUBLIC_KEY,
            &TEST_DIGEST,
            &wrong_signature
        ));

        let mut wrong_key = TEST_PUBLIC_KEY;
        wrong_key[0] ^= 0x01;
        assert!(!verify_digest_signature(
            &wrong_key,
            &TEST_DIGEST,
            &TEST_SIGNATURE
        ));
    }

    #[test]
    fn rejects_wrong_lengths_and_garbage() {
        let digest = firmware_sha256(b"firmware image");
        assert!(!verify_firmware_digest_signature(&digest, &[0u8; 10]));
        assert!(!verify_firmware_digest_signature(&digest, &[0u8; 64]));
    }

    #[test]
    fn hasher_matches_one_shot_digest() {
        let mut hasher = FirmwareHasher::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        assert_eq!(hasher.finalize(), firmware_sha256(b"hello world"));
    }

    #[test]
    fn embedded_key_is_well_formed() {
        assert_eq!(FIRMWARE_PUBLIC_KEY.len(), FIRMWARE_PUBLIC_KEY_LEN);
        assert!(PublicKey::from_slice(&FIRMWARE_PUBLIC_KEY).is_ok());
    }
}
