//! [Datasheet STS4x](https://sensirion.com/media/documents/D2D0B4A9/67AA0F30/HT_DS_Datasheet_STS4x.pdf)

pub mod model_addrs;

use byteorder::{ByteOrder, LittleEndian};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, signal_to_temp};

use crate::{
    make_sensor,
    sensiron::generic::{Precision, Result},
};

// TODO: parse the raw returned data from commands

make_sensor!(Sts4x, "the `STS4x` temperature sensor");

impl<I: Instance> Sts4x<'_, I> {
    /// Returns the temperature in degrees celsius.
    pub async fn measure(&mut self, precision: Precision) -> Result<i32> {
        let data = self.0.measure(precision).await?;

        // datasheet section 4.4
        let temp = &data[0..=1];
        let sum = data[2];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);
        let calc_sum = crc.checksum(temp);

        // FIXME: should we return an error instead?
        if sum != calc_sum {
            defmt::warn!(
                "temp checksum did not match (ours: {:#x} != sensor's: {:#x})",
                calc_sum,
                sum
            );
        }

        let temp: u16 = LittleEndian::read_u16(temp);
        let temp_c = signal_to_temp(temp);

        Ok(temp_c)
    }
}
