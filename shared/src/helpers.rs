use alloc::vec::Vec;

/// Appends a length-prefixed string field into a raw byte buffer.
/// Payload format: [1-byte length] [UTF-8 string bytes]
pub fn append_field(buf: &mut Vec<u8>, val: &str) -> Result<(), &'static str> {
    if val.len() > 255 {
        return Err("Field length exceeds max payload limit of 255 bytes");
    }
    buf.push(val.len() as u8);
    buf.extend_from_slice(val.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_append_field_success() {
        let mut buf = vec![0x01];
        append_field(&mut buf, "test").unwrap();
        assert_eq!(buf, vec![0x01, 4, b't', b'e', b's', b't']);
    }

    #[test]
    fn test_append_empty_field() {
        let mut buf = vec![];
        append_field(&mut buf, "").unwrap();
        assert_eq!(buf, vec![0]);
    }

    #[test]
    fn test_append_field_max_len() {
        let mut buf = vec![];
        let val = "a".repeat(255);
        assert!(append_field(&mut buf, &val).is_ok());
        assert_eq!(buf.len(), 256);
        assert_eq!(buf[0], 255);
    }

    #[test]
    fn test_append_field_too_long() {
        let mut buf = vec![];
        let val = "a".repeat(256);
        let err = append_field(&mut buf, &val);
        assert!(err.is_err());
    }
}
