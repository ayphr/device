use alloc::string::{String, ToString};
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

/// Reads a length-prefixed string field starting at `cursor`, advancing it on
/// success. Returns `None` when the payload is truncated or the bytes are not
/// valid UTF-8.
pub fn read_field(data: &[u8], cursor: &mut usize) -> Option<String> {
    if *cursor >= data.len() {
        return None;
    }
    let len = data[*cursor] as usize;
    *cursor += 1;
    if *cursor + len > data.len() {
        return None;
    }
    let val = core::str::from_utf8(&data[*cursor..*cursor + len])
        .ok()?
        .to_string();
    *cursor += len;
    Some(val)
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

    #[test]
    fn test_read_field_round_trip() {
        let mut buf = vec![0x01];
        append_field(&mut buf, "geo").unwrap();
        append_field(&mut buf, "").unwrap();

        let mut cursor = 1;
        assert_eq!(read_field(&buf, &mut cursor).as_deref(), Some("geo"));
        assert_eq!(read_field(&buf, &mut cursor).as_deref(), Some(""));
        assert_eq!(cursor, buf.len());
    }

    #[test]
    fn test_read_field_empty_buffer() {
        let mut cursor = 0;
        assert_eq!(read_field(&[], &mut cursor), None);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_read_field_truncated_length() {
        let buf = [5, b'a', b'b'];
        let mut cursor = 0;
        assert_eq!(read_field(&buf, &mut cursor), None);
        assert_eq!(cursor, 1, "cursor only advances past the length byte");
    }

    #[test]
    fn test_read_field_invalid_utf8() {
        let buf = [2, 0xFF, 0xFE];
        let mut cursor = 0;
        assert_eq!(read_field(&buf, &mut cursor), None);
    }
}
