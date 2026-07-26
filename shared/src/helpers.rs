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
