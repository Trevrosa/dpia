//! a generic sensiron sensor. works for `SHT4x` and `STS4x`.

use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, concat_bytes};
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c;

use crate::sensiron::sum_check;

/// A generic Sensiron sensor. A custom implementation can be created with the macro `make_sensor!(NAME, DOCS, MAX_SIZE)`.
///
/// `MAX_SIZE` is the max size (in bytes) of the return data (e.g. `6` for `SHT4x` and `STS4x`).
#[derive(Clone, Copy)]
pub struct Sensor {
    addr: u8,
}

impl Sensor {
    /// Create a new sensor instance.
    pub fn new(addr: u8) -> Self {
        Self { addr }
    }

    /// Send a command to the sensor and get its response.
    ///
    /// `RESULT_SIZE` is in bytes.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub(super) async fn run_cmd<I: i2c::I2c, const RESULT_SIZE: usize>(
        &self,
        bus: &mut I,
        cmd: impl Into<u16>,
        read_delay: Option<Duration>,
    ) -> Result<[u8; RESULT_SIZE], I::Error> {
        let mut result = [0; RESULT_SIZE];
        self.start_cmd(bus, cmd).await?;
        Timer::after(read_delay.unwrap_or_default()).await;
        bus.read(self.addr, &mut result).await?;
        Ok(result)
    }

    pub(super) async fn start_cmd<I: i2c::I2c>(
        &self,
        bus: &mut I,
        cmd: impl Into<u16>,
    ) -> Result<(), I::Error> {
        bus.write(self.addr, &cmd.into().to_be_bytes()).await
    }

    /// Run the measure command with the provided [`Precision`] and get its response. (only works for `SHT4x` and `STS4x`)
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub(super) async fn measure<I: i2c::I2c, const RESULT_SIZE: usize>(
        &self,
        bus: &mut I,
        precision: Precision,
    ) -> Result<[u8; RESULT_SIZE], I::Error> {
        let cmd = precision.cmd();
        self.run_cmd(bus, cmd, Some(precision.timing())).await
    }

    /// Read the serial number of the sensor. (sht4x and sts4x only)
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub(super) async fn serial_num<I: i2c::I2c>(
        &self,
        bus: &mut I,
        cmd: u8,
    ) -> Result<u32, I::Error> {
        let data: [u8; 6] = self.run_cmd(bus, cmd, None).await?;

        let serial = &data[0..=1];
        let sum = data[2];
        let serial1 = &data[3..=4];
        let sum1 = data[5];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, serial, sum, "serial");
        sum_check(&crc, serial1, sum1, "serial1");

        let combined = concat_bytes!(serial, serial1, 4);

        Ok(u32::from_be_bytes(combined))
    }

    /// Tell the sensor to soft-reset and wait. (sht4x and sts4x only)
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub(super) async fn soft_reset<I: i2c::I2c>(&self, bus: &mut I) -> Result<(), I::Error> {
        const SOFT_RESET: u8 = 0x94;
        // special command, only ACKs, so no return data
        self.start_cmd(bus, SOFT_RESET).await?;
        Timer::after_millis(1).await;
        Ok(())
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
    pub fn cmd(&self) -> u8 {
        // datasheet section 4.4
        match self {
            Precision::High => 0xFD,
            Precision::Medium => 0xF6,
            Precision::Low => 0xE0,
        }
    }

    /// The max time it takes for a measurement at the corresponding precision.
    pub fn timing(&self) -> Duration {
        // datasheet section 3.1
        Duration::from_micros(match self {
            Precision::High => 8300,
            Precision::Medium => 4500,
            Precision::Low => 1600,
        })
    }
}
