#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    InvalidCommand = 0x01,
    AuthenticationRequired = 0x02,
    InvalidPayload = 0x03,
    NvsWriteFailed = 0x04,
    SensorUnavailable = 0x05,
    Unknown = 0xFF,
}

impl DeviceError {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0x01 => Self::InvalidCommand,
            0x02 => Self::AuthenticationRequired,
            0x03 => Self::InvalidPayload,
            0x04 => Self::NvsWriteFailed,
            0x05 => Self::SensorUnavailable,
            _ => Self::Unknown,
        }
    }
}
