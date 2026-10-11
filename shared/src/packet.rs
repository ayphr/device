use core::fmt;

pub const PACKET_STX: u8 = 0x02;
pub const PACKET_ETX: u8 = 0x03;
pub const PACKET_LEN: usize = 18;
pub const MAX_PACKET_TYPE: u8 = 0x0f;
pub const PACKET_TYPE_SENSOR: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketError {
    PacketType,
    Temperature,
    Humidity,
    AirPressure,
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PacketError::PacketType => "Packet type must be between 0 and 15",
            PacketError::Temperature => "Packet temperature must be between -150 and 150 °C",
            PacketError::Humidity => "Packet humidity must be between 0 and 100 %",
            PacketError::AirPressure => "Packet air pressure must be between 300 and 1100 hPa",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorPacket {
    /// Packet type, a 4-bit unsigned integer.
    pub packet_type: u8,
    /// Temperature in Celsius, `-150.00..=150.00` with 0.01 precision.
    pub temperature: f32,
    /// Relative humidity as a percentage, `0.0..=100.0` with 0.1 precision.
    pub humidity: f32,
    /// Air pressure in hectopascals, `300..=1100` with 1 hPa precision.
    pub air_pressure: u16,

    pub serial: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnpackedSensorPacket {
    pub packet: SensorPacket,
    pub valid: bool,
}

pub fn validate_packet(packet: &SensorPacket) -> Result<(), PacketError> {
    if packet.packet_type > MAX_PACKET_TYPE {
        return Err(PacketError::PacketType);
    }
    if !(-150.0..=150.0).contains(&packet.temperature) {
        return Err(PacketError::Temperature);
    }
    if !(0.0..=100.0).contains(&packet.humidity) {
        return Err(PacketError::Humidity);
    }
    if !(300..=1100).contains(&packet.air_pressure) {
        return Err(PacketError::AirPressure);
    }
    Ok(())
}

pub fn pack(packet: &SensorPacket) -> Result<[u8; PACKET_LEN], PacketError> {
    validate_packet(packet)?;

    let temperature = scale_round(packet.temperature + 150.0, 100.0) as u128;
    let humidity = scale_round(packet.humidity, 10.0) as u128;
    let air_pressure = (packet.air_pressure - 300) as u128;

    let mut raw: u128 = 0;
    raw |= (packet.packet_type as u128) & 0x0f;
    raw |= (temperature & 0x7fff) << 4;
    raw |= (humidity & 0x03ff) << 19;
    raw |= (air_pressure & 0x03ff) << 29;
    raw |= (packet.serial as u128) << 95;

    if !raw.count_ones().is_multiple_of(2) {
        raw |= 1u128 << 127;
    }

    let mut out = [0u8; PACKET_LEN];
    out[0] = PACKET_STX;
    out[1..9].copy_from_slice(&((raw >> 64) as u64).to_be_bytes());
    out[9..17].copy_from_slice(&(raw as u64).to_be_bytes());
    out[17] = PACKET_ETX;
    Ok(out)
}

pub fn unpack(raw: &[u8]) -> Option<UnpackedSensorPacket> {
    if raw.len() != PACKET_LEN || raw[0] != PACKET_STX || raw[PACKET_LEN - 1] != PACKET_ETX {
        return None;
    }

    let mut high_bytes = [0u8; 8];
    high_bytes.copy_from_slice(&raw[1..9]);
    let mut low_bytes = [0u8; 8];
    low_bytes.copy_from_slice(&raw[9..17]);

    let high = u64::from_be_bytes(high_bytes) as u128;
    let low = u64::from_be_bytes(low_bytes) as u128;
    let payload = (high << 64) | low;

    let packet_type = (payload & 0x0f) as u8;
    let temperature =
        round_decimals(((payload >> 4) & 0x7fff) as f64 / 100.0 - 150.0, 100.0) as f32;
    let humidity = round_decimals(((payload >> 19) & 0x03ff) as f64 / 10.0, 10.0) as f32;
    let air_pressure = (((payload >> 29) & 0x03ff) as u16) + 300;
    let serial = ((payload >> 95) & 0xffff_ffff) as u32;

    Some(UnpackedSensorPacket {
        packet: SensorPacket {
            packet_type,
            temperature,
            humidity,
            air_pressure,
            serial,
        },
        valid: payload.count_ones().is_multiple_of(2),
    })
}

fn scale_round(value: f32, scale: f32) -> i64 {
    let scaled = value * scale;
    if scaled >= 0.0 {
        (scaled + 0.5) as i64
    } else {
        (scaled - 0.5) as i64
    }
}

fn round_decimals(value: f64, factor: f64) -> f64 {
    let scaled = value * factor;
    let rounded = if scaled >= 0.0 {
        (scaled + 0.5) as i64
    } else {
        (scaled - 0.5) as i64
    };
    rounded as f64 / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> [u8; PACKET_LEN] {
        let mut out = [0u8; PACKET_LEN];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }

    fn packet(temperature: f32, humidity: f32, air_pressure: u16, serial: u32) -> SensorPacket {
        SensorPacket {
            packet_type: PACKET_TYPE_SENSOR,
            temperature,
            humidity,
            air_pressure,
            serial,
        }
    }

    #[test]
    fn packs_known_vectors() {
        let vectors = [
            (
                packet(20.0, 50.0, 1013, 12345),
                "028000181c80000000000000592fa4268103",
            ),
            (
                packet(-150.0, 0.0, 300, 0),
                "028000000000000000000000000000000103",
            ),
            (
                packet(150.0, 100.0, 1100, 0xffff_ffff),
                "02ffffffff80000000000000641f47530103",
            ),
            (
                packet(-5.25, 37.5, 999, 42),
                "028000001500000000000000576bbb88b103",
            ),
        ];

        for (packet, expected) in vectors {
            assert_eq!(pack(&packet).unwrap(), bytes(expected), "{packet:?}");
        }
    }

    #[test]
    fn round_trips_values_and_reports_valid_parity() {
        for packet in [
            packet(20.0, 50.0, 1013, 12345),
            packet(-150.0, 0.0, 300, 0),
            packet(150.0, 100.0, 1100, 0xffff_ffff),
            packet(-5.25, 37.5, 999, 42),
        ] {
            let decoded = unpack(&pack(&packet).unwrap()).unwrap();
            assert!(decoded.valid);
            assert_eq!(decoded.packet, packet);
        }
    }

    #[test]
    fn detects_a_flipped_bit() {
        let mut raw = pack(&packet(20.0, 50.0, 1013, 12345)).unwrap();
        raw[10] ^= 0x01;
        let decoded = unpack(&raw).unwrap();
        assert!(!decoded.valid);
    }

    #[test]
    fn rejects_bad_framing_and_length() {
        let good = pack(&packet(20.0, 50.0, 1013, 12345)).unwrap();

        assert!(unpack(&good[..17]).is_none());

        let mut bad_stx = good;
        bad_stx[0] = 0x00;
        assert!(unpack(&bad_stx).is_none());

        let mut bad_etx = good;
        bad_etx[PACKET_LEN - 1] = 0x00;
        assert!(unpack(&bad_etx).is_none());
    }

    #[test]
    fn validates_field_ranges() {
        let mut out_of_range = packet(20.0, 50.0, 1013, 1);
        out_of_range.packet_type = 16;
        assert_eq!(validate_packet(&out_of_range), Err(PacketError::PacketType));

        out_of_range = packet(150.01, 50.0, 1013, 1);
        assert_eq!(
            validate_packet(&out_of_range),
            Err(PacketError::Temperature)
        );

        out_of_range = packet(20.0, 100.1, 1013, 1);
        assert_eq!(validate_packet(&out_of_range), Err(PacketError::Humidity));

        out_of_range = packet(20.0, 50.0, 299, 1);
        assert_eq!(
            validate_packet(&out_of_range),
            Err(PacketError::AirPressure)
        );

        assert!(validate_packet(&packet(-150.0, 0.0, 300, 0)).is_ok());
        assert!(validate_packet(&packet(150.0, 100.0, 1100, 0xffff_ffff)).is_ok());
    }

    #[test]
    fn pack_rejects_invalid_packets() {
        let mut invalid = packet(20.0, 50.0, 1013, 1);
        invalid.humidity = -0.1;
        assert_eq!(pack(&invalid), Err(PacketError::Humidity));
    }
}
