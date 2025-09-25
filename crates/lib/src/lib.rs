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

/// Takes two slices and the combined length of the slices. Creates a new slice `[T; combined_len]`.
#[macro_export]
macro_rules! concat_bytes {
    ($slice:expr, $slice1:expr, $combined_len:expr) => {{
        debug_assert!(
            $combined_len >= $slice.len() + $slice1.len(),
            "`combined_len` must fit `slice` and `slice1`"
        );
        let mut combined = [0; $combined_len];
        for (i, byte) in $slice.iter().enumerate() {
            combined[i] = *byte;
        }
        for (i, byte) in $slice1.iter().enumerate() {
            combined[($combined_len / 2) + i] = *byte;
        }
        combined
    }};
}

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
    #[should_panic]
    fn concat_bytes_wrong_len() {
        concat_bytes!([1], [2], 0);
    }

    #[test]
    fn concat_bytes() {
        let empty: [u8; 0] = [];
        assert_eq!(concat_bytes!(empty, empty, 0), []);
        assert_eq!(concat_bytes!([1], [2], 2), [1, 2]);
        assert_eq!(concat_bytes!([1, 2], [3, 4], 4), [1, 2, 3, 4]);
    }

    #[test]
    fn signal() {
        let rh = signal_to_rh(0xBEEF);

        assert_ne!(rh, 0);
    }
}
