//! a generic sensiron sensor. works for `SHT4x` and `STS4x`.

use embassy_rp::Peri;
use embassy_rp::i2c::{self, Async, Config, I2c, Instance, InterruptHandler, SclPin, SdaPin};
use embassy_rp::interrupt::typelevel::Binding;

pub enum Precision {
    High,
    Medium,
    Low,
}

impl Precision {
    pub fn cmd(self) -> u8 {
        match self {
            Precision::High => 0xFD,
            Precision::Medium => 0xF6,
            Precision::Low => 0xE0,
        }
    }
}

/// Uses async i2c.
pub struct Sensor<'a, I: Instance> {
    bus: I2c<'a, I, Async>,
    addr: u8,
}

pub type Result<T> = core::result::Result<T, i2c::Error>;

impl<'d, I: Instance> Sensor<'d, I> {
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
        let bus = I2c::new_async(peri, scl, sda, irq, config);
        defmt::info!("initialised i2c bus!");
        Self { bus, addr }
    }

    // the max return size is 6 bytes (2 * 8-bit T-data; 8-bit CRC; 2 * 8-bit RH-data; 8-bit CRC).
    async fn run_cmd(&mut self, cmd: u8) -> Result<[u8; 6]> {
        let mut result = [0; 6];
        self.bus
            .write_read_async(self.addr, [cmd], &mut result)
            .await?;
        Ok(result)
    }

    pub async fn measure(&mut self, precision: Precision) -> Result<[u8; 6]> {
        let cmd = precision.cmd();
        self.run_cmd(cmd).await
    }

    pub async fn serial_num(&mut self) -> Result<[u8; 6]> {
        const READ_SERIAL_NUMBER: u8 = 0x89;
        self.run_cmd(READ_SERIAL_NUMBER).await
    }

    // special command, only ACKs
    pub async fn soft_reset(&mut self) -> Result<()> {
        const SOFT_RESET: u8 = 0x94;
        self.bus.write_async(self.addr, [SOFT_RESET]).await
        // TODO: should we wait a bit here?
    }

    // ..heating cmds
}
