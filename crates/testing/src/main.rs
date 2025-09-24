#[cfg(test)]
mod tests {
    use dpia_lib::CRC_8_SENSIRON;

    #[test]
    fn crc() {
        let crc = crc::Crc::<u8>::new(&CRC_8_SENSIRON);
        let sum = crc.checksum(&[0xBE, 0xEF]);

        assert_eq!(sum, 0x92);
    }
}

fn main() {
    println!("Hello, World!");
}
