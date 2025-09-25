#![cfg_attr(not(test), no_std)]

use crc::Algorithm;

// STS4x section 4.3, same for SHT4x
/// The CRC-8 algorithm Sensiron uses (at least for the `STS4x` and `SHT4x`).
pub const CRC_8_SENSIRON: Algorithm<u8> = Algorithm {
    width: 8,
    poly: 0x31,
    init: 0xFF,
    refin: false,
    refout: false,
    xorout: 0x00,
    check: 0x00,   // not specified
    residue: 0x00, // not specified
};

// FIXME: is this correct?
pub fn signal_to_rh(data: u16) -> u16 {
    119 * (data / u16::MAX)
}

// FIXME: is this correct?
pub fn signal_to_temp(data: u16) -> i32 {
    130 * (i32::from(data) / i32::from(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc() {
        let crc = crc::Crc::<u8>::new(&CRC_8_SENSIRON);
        let sum = crc.checksum(&[0xBE, 0xEF]);

        assert_eq!(sum, 0x92);
    }

    #[test]
    fn signal() {
        let rh = signal_to_rh(0xBEEF);

        assert_ne!(rh, 0);
    }
}
