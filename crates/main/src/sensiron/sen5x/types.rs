use crate::sensiron::is_bit_set;

#[derive(defmt::Format)]
pub struct Measurement {
    pub pm10: u16,
    pub pm2_5: u16,
    pub voc: u16,
    pub nox: u16,
}

#[derive(defmt::Format)]
pub struct DeviceStatus {
    /// See [`FanSpeed`]
    pub fan_speed: FanSpeed,
    /// See [`FanCleaning`]
    pub fan_cleaning: FanCleaning,
    /// - The fan is checked once per second in the measurement mode. If 0 RPM is measured twice in succession,
    ///   the FAN bit is set.
    /// - The FAN-bit will not be cleared automatically.
    /// - A fan failure can occur if the fan is mechanically blocked or broken.
    pub fan_failure: bool,
    /// See [`GasSensor`]
    pub gas_sensor: GasSensor,
    /// See [`Rht`]
    pub rht: Rht,
    /// See [`Laser`]
    pub laser: Laser,
}

impl DeviceStatus {
    pub fn from_u32(data: u32) -> Self {
        DeviceStatus {
            fan_speed: if is_bit_set(data, 21) {
                FanSpeed::TooHighOrLow
            } else {
                FanSpeed::Ok
            },
            fan_cleaning: if is_bit_set(data, 19) {
                FanCleaning::Cleaning
            } else {
                FanCleaning::Normal
            },
            gas_sensor: if is_bit_set(data, 7) {
                GasSensor::Error
            } else {
                GasSensor::Normal
            },
            rht: if is_bit_set(data, 6) {
                Rht::Error
            } else {
                Rht::Normal
            },
            laser: if is_bit_set(data, 5) {
                Laser::CurrentOutOfRange
            } else {
                Laser::CurrentOk
            },
            fan_failure: is_bit_set(data, 4),
        }
    }
}

/// - During the first 3 seconds after starting the measurement (fan start-up) the fan speed is not checked.
/// - The fan speed is also not checked during the auto cleaning procedure.
/// - Apart from the two exceptions mentioned above, the fan speed is checked once per second in the
///   measurement mode. If it is out of range twice in succession, the SPEED-bit is set.
/// - At very high or low ambient temperatures, the fan may take longer to reach its target speed after start-up. In
///   this case, the bit will be set. As soon as the target speed is reached, this bit is cleared automatically.
/// - If this bit is constantly set, this indicates a problem with the power supply or that the fan is no longer working
///   properly.
#[derive(defmt::Format)]
pub enum FanSpeed {
    Ok,
    TooHighOrLow,
}

#[derive(defmt::Format)]
pub enum FanCleaning {
    Normal,
    /// Set during the automatic cleaning procedure
    Cleaning,
}

/// VOC & `NOx`
#[derive(defmt::Format)]
pub enum GasSensor {
    Normal,
    Error,
}

/// RHT communication error
#[derive(defmt::Format)]
pub enum Rht {
    Normal,
    /// Error in internal communication with the RHT sensor
    Error,
}

/// - The laser current is checked once per second in the measurement mode. If it is out of range, the LASER-bit
///   is set.
/// - If the laser current is back within limits, this bit will be not cleared automatically.
/// - A laser failure can occur at very high temperatures outside of specifications or when the laser module is
///   defective.
#[derive(defmt::Format)]
pub enum Laser {
    CurrentOk,
    CurrentOutOfRange,
}
