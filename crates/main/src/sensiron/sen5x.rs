use byteorder::{BigEndian, ByteOrder};
use crc::Crc;
use dpia_lib::CRC_8_SENSIRON;
use embassy_time::Timer;

use super::generic::Result;

use crate::{make_sensor, sensiron::sum_check};

pub const ADDR: u8 = 0x69;

make_sensor!(Sen5x, "the `SEN5x` particulate matter sensor", 48);

#[derive(defmt::Format)]
pub struct Measurement {
    pm10: u16,
    pm2_5: u16,
    voc: u16,
    nox: u16,
}

// sen5x uses big-endian, max size from the read serial number command in datasheet section 6.1.15
impl<I: Instance> Sen5x<'_, I> {
    /// Read the measured values of the sensor.
    ///
    /// Note: this takes longer to run because we need to put the sensor in measurement mode,
    /// wait for the measurement to be taken, read the result, then put the sensor back in idle mode.
    ///
    /// # Errors
    ///
    /// Will error if there is an I2c error.
    pub async fn measure(&mut self) -> Result<Measurement> {
        // start measurement, takes 50 ms (datasheet 6.1)
        self.0.write_cmd([0, 0x21]).await?;
        Timer::after_millis(50).await;

        // wait until measurement is ready
        loop {
            if self.data_ready().await.is_ok_and(|r| r) {
                break;
            }
            Timer::after_secs(3).await;
        }

        // format: 2 bytes data, 1 byte crc, repeated for each measurement (24 bytes total)
        let data = self.0.run_cmd([0x03, 0xC4]).await?;

        // stop measurement to save power (datasheet 6.1.3)
        self.0.write_cmd([0x01, 0x04]).await?;

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        // ignore pm1_0 and pm4_0 for now

        let pm2_5 = &data[3..=4];
        let pm2_5_sum = data[5];

        let pm10 = &data[9..=10];
        let pm10_sum = data[11];

        sum_check(&crc, pm2_5, pm2_5_sum, "pm2.5");
        sum_check(&crc, pm10, pm10_sum, "pm10");

        let voc = &data[18..=19];
        sum_check(&crc, voc, data[20], "voc");

        let nox = &data[21..=22];
        sum_check(&crc, nox, data[23], "nox");

        Ok(Measurement {
            pm10: BigEndian::read_u16(pm10),
            pm2_5: BigEndian::read_u16(pm2_5),
            voc: BigEndian::read_u16(voc),
            nox: BigEndian::read_u16(nox),
        })
    }

    async fn data_ready(&mut self) -> Result<bool> {
        // format: 1 unused byte, 1 byte data, 1 byte crc of bytes 0..=1
        let data = self.0.run_cmd([0x02, 0x02]).await?;

        let ready = &data[0..=1];
        let sum = data[2];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);
        sum_check(&crc, ready, sum, "data ready flag");

        Ok(ready[1] == 1)
    }

    /// Returns the serial number of the sensor as an ascii buffer
    pub async fn serial_num(&mut self) -> Result<[u8; 32]> {
        // format: 2 bytes ascii, 1 byte checksum, ... repeated to byte 47
        let data = self.0.run_cmd([0xD0, 0x33]).await?;

        let mut serial = [0; 32];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);
        for i in 0..16 {
            // move in 3 byte chunks
            let chunk = &data[i * 3..i * 3 + 3];

            let ascii = &chunk[0..=1];
            let sum = chunk[2];

            sum_check(&crc, ascii, sum, "ascii");

            // for every 3 bytes, we get 2 ascii bytes
            serial[i * 2..i * 2 + 2].copy_from_slice(ascii);
        }

        Ok(serial)
    }

    pub async fn reset(&mut self) -> Result<()> {
        // reset command, takes 100 ms (datasheet 6.1)
        self.0.write_cmd([0xD3, 0x04]).await?;
        Timer::after_millis(100).await;
        Ok(())
    }

    /// use `reset` instead
    pub async fn soft_reset(&mut self) -> ! {
        unimplemented!("soft reset is not supported on sen5x, use reset instead")
    }
}
