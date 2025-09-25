#![no_std]

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc() {
        let crc = crc::Crc::<u8>::new(&CRC_8_SENSIRON);
        let sum = crc.checksum(&[0xBE, 0xEF]);

        assert_eq!(sum, 0x92);
    }
}