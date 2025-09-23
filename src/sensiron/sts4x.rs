//! [Datasheet STS4x](https://sensirion.com/media/documents/D2D0B4A9/67AA0F30/HT_DS_Datasheet_STS4x.pdf)

pub mod model_addrs;

use crate::make_sensor;

// TODO: parse the raw returned data from commands

make_sensor!(Sts4x, "asd");
