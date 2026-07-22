use byteorder::{BigEndian, ByteOrder};
use crc::Crc;
use dpia_lib::CRC_8_SENSIRON;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c;

use crate::{make_sensor, sensiron::sum_check};

pub const ADDR: u8 = 0x69;

make_sensor!(Sen5x, "the `SEN5x` particulate matter sensor");

#[derive(defmt::Format)]
pub struct Measurement {
    pub pm10: u16,
    pub pm2_5: u16,
    pub voc: u16,
    pub nox: u16,
}

// sen5x uses big-endian, max size from the read serial number command in datasheet section 6.1.15
impl Sen5x {
    /// Read the measured values of the sensor.
    ///
    /// Note: this takes longer to run because we need to put the sensor in measurement mode,
    /// wait for the measurement to be taken, read the result, then put the sensor back in idle mode.
    ///
    /// # Errors
    ///
    /// Fails if there is an I2c error.
    pub async fn measure<I: i2c::I2c>(&self, bus: &mut I) -> Result<Measurement, I::Error> {
        // start measurement, takes 50 ms (datasheet 6.1)
        self.0.write_cmd(bus, 0x21_u8).await?;
        Timer::after_millis(50).await;

        // wait until measurement is ready
        loop {
            if self.data_ready(bus).await.is_ok_and(|r| r) {
                break;
            }
            Timer::after_secs(1).await;
        }

        // format: 2 bytes data, 1 byte crc, repeated for each measurement (24 bytes total)
        // read_delay from datasheet 6.1
        let data: [u8; 24] = self
            .0
            .run_cmd(bus, 0x03C4_u16, Some(Duration::from_millis(20)))
            .await?;

        // stop measurement to save power (datasheet 6.1.3), takes 200 ms (datasheet 6.1)
        self.0.write_cmd(bus, 0x0104_u16).await?;
        Timer::after_millis(200).await;

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

    async fn data_ready<I: i2c::I2c>(&self, bus: &mut I) -> Result<bool, I::Error> {
        // format: 1 unused byte, 1 byte data, 1 byte crc of bytes 0..=1
        // read_delay from datasheet 6.1
        let data: [u8; 3] = self
            .0
            .run_cmd(bus, 0x0202_u16, Some(Duration::from_millis(20)))
            .await?;

        let ready = &data[0..=1];
        let sum = data[2];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);
        sum_check(&crc, ready, sum, "data ready flag");

        Ok(ready[1] == 1)
    }

    /// Returns the serial number of the sensor as an ascii buffer
    ///
    /// # Errors
    ///
    /// Fails on an I2c error.
    pub async fn serial_num<I: i2c::I2c>(&self, bus: &mut I) -> Result<[u8; 32], I::Error> {
        // format: 2 bytes ascii, 1 byte checksum, ... repeated to byte 48
        let data: [u8; 48] = self
            .0
            .run_cmd(bus, 0xD033_u16, Some(Duration::from_millis(20)))
            .await?;

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

    /// Resets the sensor.
    ///
    /// # Errors
    ///
    /// Fails on an I2c error.
    pub async fn reset<I: i2c::I2c>(&self, bus: &mut I) -> Result<(), I::Error> {
        // reset command, takes 100 ms (datasheet 6.1)
        self.0.write_cmd(bus, 0xD304_u16).await?;
        Timer::after_millis(100).await;
        Ok(())
    }
}
