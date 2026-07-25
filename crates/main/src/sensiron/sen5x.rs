use byteorder::{BigEndian, ByteOrder};
use crc::Crc;
use dpia_lib::{CRC_8_SENSIRON, concat_bytes};
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c;

use super::sum_check;
use crate::make_sensor;

pub mod types;
use types::{DeviceStatus, Measurement};

pub const ADDR: u8 = 0x69;

make_sensor!(Sen5x, "the `SEN5x` particulate matter sensor");

impl Sen5x {
    /// Get the device's status.
    ///
    /// # Errors
    ///
    /// Fails if there is an I2c error.
    pub async fn status<I: i2c::I2c>(&self, bus: &mut I) -> Result<DeviceStatus, I::Error> {
        const READ_DEVICE_STATUS: u16 = 0xD206;
        let data: [u8; 6] = self
            .0
            .run_cmd(
                bus,
                READ_DEVICE_STATUS.to_be_bytes(),
                Some(Duration::from_millis(20)),
            )
            .await?;

        let msb = &data[0..=1];
        let msb_crc = data[2];
        let lsb = &data[3..=4];
        let lsb_crc = data[5];

        let crc = Crc::<u8>::new(&CRC_8_SENSIRON);

        sum_check(&crc, msb, msb_crc, "msb");
        sum_check(&crc, lsb, lsb_crc, "lsb");

        let data = u32::from_be_bytes(concat_bytes!(msb, lsb, 4));

        Ok(DeviceStatus::from_u32(data))
    }

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
        const START_MEASUREMENT: u8 = 0x21;
        self.0.start_cmd(bus, [START_MEASUREMENT]).await?;
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
        const READ_MEASURED_VALUES: u16 = 0x03C4;
        let data: [u8; 24] = self
            .0
            .run_cmd(
                bus,
                READ_MEASURED_VALUES.to_be_bytes(),
                Some(Duration::from_millis(20)),
            )
            .await?;

        // stop measurement to save power (datasheet 6.1.3), takes 200 ms (datasheet 6.1)
        const STOP_MEASUREMENT: u16 = 0x0104;
        self.0
            .start_cmd(bus, STOP_MEASUREMENT.to_be_bytes())
            .await?;

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
        const READ_DATA_READY: u16 = 0x0202;
        // format: 1 unused byte, 1 byte data, 1 byte crc of bytes 0..=1
        // read_delay from datasheet 6.1
        let data: [u8; 3] = self
            .0
            .run_cmd(
                bus,
                READ_DATA_READY.to_be_bytes(),
                Some(Duration::from_millis(20)),
            )
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
            .run_cmd(
                bus,
                0xD033_u16.to_be_bytes(),
                Some(Duration::from_millis(20)),
            )
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
        self.0.start_cmd(bus, 0xD304_u16.to_be_bytes()).await?;
        Timer::after_millis(100).await;
        Ok(())
    }
}
