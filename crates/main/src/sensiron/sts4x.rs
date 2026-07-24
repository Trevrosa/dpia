//! [Datasheet STS4x](https://sensirion.com/media/documents/D2D0B4A9/67AA0F30/HT_DS_Datasheet_STS4x.pdf)

pub mod model_addrs;

use byteorder::{BigEndian, ByteOrder};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, signal_to_temp};
use embedded_hal_async::i2c;

use crate::{
    make_sensor,
    sensiron::{generic::Precision, sum_check},
};

make_sensor!(Sts4x, "the `STS4x` temperature sensor");

impl Sts4x {
    /// Returns the temperature in degrees celsius.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn measure<I: i2c::I2c>(
        &self,
        bus: &mut I,
        precision: Precision,
    ) -> Result<f32, I::Error> {
        let data: [u8; 3] = self.0.measure(bus, precision).await?;

        // datasheet section 4.4
        let temp = &data[0..=1];
        let sum = data[2];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, temp, sum, "temperature");

        let temp: u16 = BigEndian::read_u16(temp);
        let temp_c = signal_to_temp(temp);

        Ok(temp_c)
    }

    /// Read the serial number of the sensor.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn serial_num<I: i2c::I2c>(&self, bus: &mut I) -> Result<u32, I::Error> {
        const READ_SERIAL_NUMBER: u8 = 0x89;
        self.0.serial_num(bus, READ_SERIAL_NUMBER).await
    }

    /// Tell the sensor to soft-reset and wait.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn soft_reset<I: i2c::I2c>(&self, bus: &mut I) -> Result<(), I::Error> {
        self.0.soft_reset(bus).await
    }
}
