//! [Datasheet SHT4x_5](https://sensirion.com/media/documents/33FD6951/67EB9032/HT_DS_Datasheet_SHT4x_5.pdf)

pub mod model_addrs;

use byteorder::{ByteOrder, LittleEndian};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, signal_to_rh, signal_to_temp};

use crate::{
    make_sensor,
    sensiron::generic::{Precision, Result},
};

// TODO: parse the raw returned data from commands

#[derive(defmt::Format)]
pub struct Measurement {
    relative_humidity: u16,
    /// in degrees celsius.
    temperature: i32,
}

make_sensor!(Sht4x, "the `SHT4x` temperature-and-humidty sensor");

impl<I: Instance> Sht4x<'_, I> {
    /// Returns the relative humidity as a % and temperature in degrees celsius.
    pub async fn measure(&mut self, precision: Precision) -> Result<Measurement> {
        let data = self.0.measure(precision).await?;

        // datasheet section 4.5
        let temp = &data[0..=1];
        let t_sum = data[2];
        let humidity = &data[3..=4];
        let h_sum = data[5];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);
        let t_calc_sum = crc.checksum(temp);
        let h_calc_sum = crc.checksum(humidity);

        // FIXME: should we return an error instead?
        if t_sum != t_calc_sum {
            defmt::warn!(
                "temp checksum did not match (ours: {:#x} != sensor's: {:#x})",
                t_calc_sum,
                t_sum
            );
        }
        if h_sum != h_calc_sum {
            defmt::warn!(
                "humidity checksum did not match (ours: {:#x} != sensor's: {:#x})",
                h_calc_sum,
                h_sum
            );
        }

        let temp: u16 = LittleEndian::read_u16(temp);
        let humidity: u16 = LittleEndian::read_u16(humidity);

        let temp_c = signal_to_temp(temp);
        let humidity = signal_to_rh(humidity);

        Ok(Measurement {
            relative_humidity: humidity,
            temperature: temp_c,
        })
    }
}
