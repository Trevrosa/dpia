//! [Datasheet SHT4x_5](https://sensirion.com/media/documents/33FD6951/67EB9032/HT_DS_Datasheet_SHT4x_5.pdf)

pub mod model_addrs;

use byteorder::{ByteOrder, LittleEndian};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, signal_to_rh, signal_to_temp};

use crate::{
    make_sensor,
    sensiron::{
        generic::{Precision, Result},
        sum_check,
    },
};

#[derive(defmt::Format)]
pub struct Measurement {
    relative_humidity: u16,
    /// in degrees celsius.
    temperature: u16,
}

make_sensor!(Sht4x, "the `SHT4x` temperature-and-humidty sensor");

impl<I: Instance> Sht4x<'_, I, 6> {
    /// Returns the relative humidity as a % and temperature in degrees celsius.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn measure(&mut self, precision: Precision) -> Result<Measurement> {
        let data = self.0.measure(precision).await?;

        // datasheet section 4.5
        let temp = &data[0..=1];
        let t_sum = data[2];
        let humidity = &data[3..=4];
        let h_sum = data[5];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, temp, t_sum, "temperature");
        sum_check(&crc, humidity, h_sum, "humidity");

        let temp: u16 = LittleEndian::read_u16(temp);
        let humidity: u16 = LittleEndian::read_u16(humidity);

        let temp_c = signal_to_temp(temp);
        let humidity = signal_to_rh(humidity);

        Ok(Measurement {
            relative_humidity: humidity,
            temperature: temp_c,
        })
    }

    pub async fn serial_num(&mut self) -> Result<[u8; 4]> {
        const READ_SERIAL_NUMBER: u8 = 0x89;
        self.0.serial_num(READ_SERIAL_NUMBER).await
    }
}
