//! a generic sensiron sensor. works for `SHT4x` and `STS4x`.

use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, concat_bytes};
use embassy_rp::i2c::{self, Async, I2c};

use crate::sensiron::sum_check;

/// A generic Sensiron sensor. A custom implementation can be created with the macro `make_sensor!(NAME, DOCS, MAX_SIZE)`.
///
/// `MAX_SIZE` is the max size (in bytes) of the return data (e.g. `6` for `SHT4x` and `STS4x`).
#[derive(Clone, Copy)]
pub struct Sensor<const MAX_SIZE: usize> {
    addr: u8,
}

pub type Result<T> = core::result::Result<T, i2c::Error>;

pub type I2cBus<'a, I> = I2c<'a, I, Async>;

impl<const MAX_SIZE: usize> Sensor<MAX_SIZE> {
    /// Create a new sensor instance.
    pub fn new(addr: u8) -> Self {
        Self { addr }
    }

    /// Send a command to the sensor and get its response.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub(super) async fn run_cmd<I: i2c::Instance>(
        &self,
        bus: &mut I2cBus<'_, I>,
        cmd: [u8; 2],
    ) -> Result<[u8; MAX_SIZE]> {
        let mut result = [0; MAX_SIZE];
        bus.write_read_async(self.addr, cmd, &mut result).await?;
        Ok(result)
    }

    pub(super) async fn write_cmd<I: i2c::Instance>(
        &self,
        bus: &mut I2cBus<'_, I>,
        cmd: [u8; 2],
    ) -> Result<()> {
        bus.write_async(self.addr, cmd).await
    }

    /// Run the measure command with the provided [`Precision`] and get its response. (only works for `SHT4x` and `STS4x`)
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn measure<I: i2c::Instance>(
        &self,
        bus: &mut I2cBus<'_, I>,
        precision: Precision,
    ) -> Result<[u8; MAX_SIZE]> {
        let cmd = precision.cmd();
        self.run_cmd(bus, [0, cmd]).await
    }

    /// Read the serial number of the sensor. (sht4x and sts4x only)
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn serial_num<I: i2c::Instance>(
        &self,
        bus: &mut I2cBus<'_, I>,
        cmd: u8,
    ) -> Result<[u8; 4]> {
        let data = self.run_cmd(bus, [0, cmd]).await?;

        assert!(
            MAX_SIZE >= 6,
            "MAX_SIZE must be at least 6 to read the serial number"
        );

        let serial = &data[0..=1];
        let sum = data[2];
        let serial1 = &data[3..=4];
        let sum1 = data[5];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, serial, sum, "serial");
        sum_check(&crc, serial1, sum1, "serial1");

        let combined = concat_bytes!(serial, serial1, 4);

        Ok(combined)
    }

    /// Tell the sensor to soft-reset.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn soft_reset<I: i2c::Instance>(&self, bus: &mut I2cBus<'_, I>) -> Result<()> {
        const SOFT_RESET: u8 = 0x94;
        // special command, only ACKs, so no return data
        self.write_cmd(bus, [0, SOFT_RESET]).await
        // TODO: wait a bit here?
    }

    // TODO: ..heating cmds
}

/// Measurement precision.
pub enum Precision {
    High,
    Medium,
    Low,
}

impl Precision {
    /// The corresponding i2c command.
    pub fn cmd(self) -> u8 {
        match self {
            Precision::High => 0xFD,
            Precision::Medium => 0xF6,
            Precision::Low => 0xE0,
        }
    }
}
