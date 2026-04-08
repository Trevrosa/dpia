pub mod generic;
pub mod sen5x;
pub mod sht4x;
pub mod sts4x;

#[macro_export]
macro_rules! make_sensor {
    ($name:ident, $doc:expr, $max_size:expr) => {
        use super::generic::Sensor;

        #[doc = $doc]
        pub struct $name(Sensor<$max_size>);

        impl $name {
            pub fn new(addr: u8) -> Self {
                Self(Sensor::new(addr))
            }
        }
    };
}

// FIXME: return an error instead of just warning?
pub fn sum_check(crc: &crc::Crc<u8>, data: &[u8], sum: u8, item: &'static str) {
    let calc_sum = crc.checksum(data);
    if calc_sum != sum {
        defmt::warn!(
            "{} checksum did not match (ours: {:#x} != sensor's: {:#x})",
            item,
            calc_sum,
            sum
        );
    }
}
