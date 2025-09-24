pub mod generic;
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
        pub struct $name<'d, I: Instance>(Sensor<'d, I>);

        impl<'d, I: Instance> $name<'d, I> {
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

        impl<'d, I: Instance> core::ops::Deref for $name<'d, I> {
            type Target = Sensor<'d, I>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'d, I: Instance> core::ops::DerefMut for $name<'d, I> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}
