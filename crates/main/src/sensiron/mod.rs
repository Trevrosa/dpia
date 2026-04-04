pub mod generic;
pub mod sen5x;
pub mod sht4x;
pub mod sts4x;

#[macro_export]
macro_rules! make_sensor {
    ($name:ident, $doc:expr) => {
        use embassy_rp::Peri;
        use embassy_rp::i2c::{Config, Instance, InterruptHandler, SclPin, SdaPin};
        use embassy_rp::interrupt::typelevel::Binding;

        use super::generic::Sensor;

        #[doc = $doc]
        pub struct $name<'d, I: Instance, const MAX_SIZE: usize>(Sensor<'d, I, MAX_SIZE>);

        impl<'d, I: Instance, const MAX_SIZE: usize> $name<'d, I, MAX_SIZE> {
            pub fn new<Scl, Sda, Irq>(
                peri: Peri<'d, I>,
                scl: Peri<'d, Scl>,
                sda: Peri<'d, Sda>,
                irq: Irq,
                config: Config,
                addr: u8,
            ) -> Self
            where
                Scl: SclPin<I>,
                Sda: SdaPin<I>,
                Irq: Binding<I::Interrupt, InterruptHandler<I>>,
            {
                let sensor = Sensor::new(peri, scl, sda, irq, config, addr);
                Self(sensor)
            }
        }

        impl<'d, I: Instance, const MAX_SIZE: usize> core::ops::Deref for $name<'d, I, MAX_SIZE> {
            type Target = Sensor<'d, I, MAX_SIZE>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'d, I: Instance, const MAX_SIZE: usize> core::ops::DerefMut
            for $name<'d, I, MAX_SIZE>
        {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
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
