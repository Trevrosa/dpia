//! [Datasheet STS4x](https://sensirion.com/media/documents/D2D0B4A9/67AA0F30/HT_DS_Datasheet_STS4x.pdf)

pub mod model_addrs;

use byteorder::{ByteOrder, LittleEndian};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, signal_to_temp};

use crate::{
    make_sensor,
    sensiron::{
        generic::{Precision, Result},
        sum_check,
    },
};

make_sensor!(Sts4x, "the `STS4x` temperature sensor", 6);

impl<I: Instance> Sts4x<'_, I> {
    /// Returns the temperature in degrees celsius.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn measure(&mut self, precision: Precision) -> Result<u16> {
        let data = self.0.measure(precision).await?;

        // datasheet section 4.4
        let temp = &data[0..=1];
        let sum = data[2];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, temp, sum, "temperature");

        let temp: u16 = LittleEndian::read_u16(temp);
        let temp_c = signal_to_temp(temp);

        Ok(temp_c)
    }

    pub async fn serial_num(&mut self) -> Result<[u8; 4]> {
        const READ_SERIAL_NUMBER: u8 = 0x89;
        self.0.serial_num(READ_SERIAL_NUMBER).await
    }
}
