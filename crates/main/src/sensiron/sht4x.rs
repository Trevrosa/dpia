//! [Datasheet SHT4x_5](https://sensirion.com/media/documents/33FD6951/67EB9032/HT_DS_Datasheet_SHT4x_5.pdf)

pub mod model_addrs;

use crate::make_sensor;

// TODO: parse the raw returned data from commands

make_sensor!(Sht4x, "the `SHT4x` temperature-and-humidty sensor");
