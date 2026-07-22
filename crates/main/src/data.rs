use core::fmt::Write;

use defmt::{error, info, warn};
use dpia::{
    HttpClientMutex,
    sensiron::{generic::Precision, sen5x::Sen5x, sht4x::Sht4x, sts4x::Sts4x},
};
use dpia_lib::display::{fmt_f32, fmt_pad_u8};
use embassy_rp::i2c;
use heapless::String;
use reqwless::request::Method;

use crate::tasks::{RpI2C0Async, RpMax7219};

// FIXME: could use <https://crates.io/crates/ufmt>

#[expect(
    clippy::missing_panics_doc,
    reason = "panics only come from .expect()s"
)]
pub fn show_data(data: &SensorData, displays: &mut RpMax7219<'static>) {
    let len = if let Some(air_temp) = data.air_temp {
        let (air_temp, dots, len) = fmt_f32(air_temp);
        let air_temp = air_temp.as_bytes().try_into().expect("we set it to len 8");
        if displays.write_str(0, air_temp, dots).is_err() {
            warn!("[data] failed to show temp digits");
        }

        len
    } else {
        3
    };

    if let Some(humidity) = data.humidity {
        let humidity = fmt_pad_u8(humidity, Some(len));
        let humidity = humidity.as_bytes().try_into().expect("we set it to len 8");
        if displays.write_str(0, humidity, 0).is_err() {
            warn!("[data] failed to show humidity digits");
        }
    }
}

#[derive(defmt::Format, Default, Clone, Copy)]
pub struct SensorData {
    pub air_temp: Option<f32>,
    pub ground_temp: Option<f32>,
    pub humidity: Option<u8>,
    pub nox: Option<u16>,
    pub voc: Option<u16>,
    pub pm10: Option<u16>,
    pub pm2_5: Option<u16>,
}

pub async fn collect(i2c: &mut RpI2C0Async, humid: Sht4x, temp: Sts4x, air: Sen5x) -> SensorData {
    let log_err = |err: &i2c::Error| {
        error!("[data] failed to measure: {}", err);
    };

    info!("[data] measuring with sht4x");
    let sht = humid
        .measure(i2c, Precision::Medium)
        .await
        .inspect_err(log_err)
        .ok();
    info!("[data] measuring with sts4x");
    let ground_temp = temp
        .measure(i2c, Precision::Medium)
        .await
        .inspect_err(log_err)
        .ok();
    info!("[data] measuring with sen5x");
    let air = air.measure(i2c).await.inspect_err(log_err).ok();

    SensorData {
        humidity: sht.as_ref().map(|m| m.relative_humidity),
        air_temp: sht.as_ref().map(|m| m.temperature),
        ground_temp,
        nox: air.as_ref().map(|m| m.nox),
        voc: air.as_ref().map(|m| m.voc),
        pm10: air.as_ref().map(|m| m.pm10),
        pm2_5: air.as_ref().map(|m| m.pm2_5),
    }
}

/// Submit `data` with a `client`.
///
/// # Errors
///
/// Fails if the request fails to send
pub async fn submit(
    client: &'static HttpClientMutex,
    data: &SensorData,
) -> Result<(), reqwless::Error> {
    let client = &mut *client.lock().await;

    info!("[data] submitting data");

    let query = data_to_query(data);

    let mut rx_buf = [0u8; 1024];
    let mut req = client.request(Method::POST, query.as_str()).await?;

    let resp = req.send(&mut rx_buf).await?;
    info!("[data] got {}", resp.status);

    Ok(())
}

// capacity should be enough
fn data_to_query(data: &SensorData) -> String<128> {
    let mut query = String::new();

    // 31
    query
        .push_str("https://dpia.trevrosa.dev/data?")
        .expect("should have enough capacity");

    // 13
    if let Some(humidity) = data.humidity {
        write!(&mut query, "humidity={humidity}&").expect("should have enough capacity");
    }
    // 16
    if let Some(air_temp) = data.air_temp {
        write!(&mut query, "air_temp={air_temp:.2}&").expect("should have enough capacity");
    }
    // 19
    if let Some(ground_temp) = data.ground_temp {
        write!(&mut query, "ground_temp={ground_temp:.2}&").expect("should have enough capacity");
    }
    // 10
    if let Some(nox) = data.nox {
        write!(&mut query, "nox={nox}&").expect("should have enough capacity");
    }
    // 10
    if let Some(voc) = data.voc {
        write!(&mut query, "voc={voc}&").expect("should have enough capacity");
    }
    // 11
    if let Some(pm10) = data.pm10 {
        write!(&mut query, "pm10={pm10}&").expect("should have enough capacity");
    }
    // 10
    if let Some(pm2_5) = data.pm2_5 {
        write!(&mut query, "pm25={pm2_5}").expect("should have enough capacity");
    }

    // counted length of 120

    query
}
