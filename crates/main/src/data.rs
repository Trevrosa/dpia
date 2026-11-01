use core::{fmt::Write, range::RangeInclusive, str::FromStr};

use defmt::{error, info, warn};
use dpia::{
    HttpClientMutex,
    errors::ParseError,
    sensiron::{generic::Precision, sen5x::Sen5x, sht4x::Sht4x, sts4x::Sts4x},
};
use dpia_lib::display::{fmt_f32, fmt_pad_u8};
use embassy_rp::i2c;
use heapless::{String, index_map::FnvIndexMap};
use miniserde::Deserialize;
use reqwless::request::{Method, RequestBuilder};

use crate::tasks::{MultiLed, RpI2C0Async, RpMax7219};

// FIXME: could use <https://crates.io/crates/ufmt>

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

#[derive(Default)]
pub struct SafetyDefinitions {
    /// temperature
    high: u8,
    /// temperature: rules
    inner: FnvIndexMap<u8, [Rule; 3], 8>,
}

#[derive(Clone, Copy, Default)]
pub enum Safety {
    Ok,
    Caution,
    #[default]
    Danger,
}

impl FromStr for Safety {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "ok" => Self::Ok,
            "caution" => Self::Caution,
            "danger" => Self::Danger,
            _ => return Err(()),
        };
        Ok(val)
    }
}

#[derive(Clone, Copy)]
pub struct Rule {
    level: Safety,
    humidity_range: RangeInclusive<u8>,
}

impl Rule {
    fn new() -> Self {
        Self {
            level: Safety::default(),
            humidity_range: RangeInclusive {
                start: 255,
                last: 255,
            },
        }
    }
}

impl SafetyDefinitions {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        #[derive(Deserialize)]
        struct RawDef {
            temperature: u8,
            rules: [Option<RawRule>; 5],
        }

        #[derive(Deserialize)]
        struct RawRule {
            humidity: [u8; 2],
            level: alloc::string::String,
        }

        let mut new = Self::default();

        let parsed: [Option<RawDef>; 10] = miniserde::json::from_str(input)?;
        let mut high_temp = 0;
        for parsed in parsed.into_iter().flatten() {
            if parsed.temperature > high_temp {
                high_temp = parsed.temperature;
            }

            for (i, rule) in parsed.rules.into_iter().flatten().enumerate() {
                let rule_range = RangeInclusive {
                    start: rule.humidity[0],
                    last: rule.humidity[1],
                };

                let entry = new
                    .inner
                    .entry(parsed.temperature)
                    .or_insert_with(|| [Rule::new(); 3]);

                if let Ok(existing) = entry {
                    existing[i].humidity_range = rule_range;
                    existing[i].level = rule.level.parse()?;
                } else {
                    // `parsed` can be at most length 10, so the map will never be at capacity
                    unreachable!();
                }
            }
        }

        new.high = high_temp;

        Ok(new)
    }

    pub fn safety_of(&self, temperature: f32, humidity: u8) -> Safety {
        let temperature = temperature as u8;
        assert!(temperature < 100);
        let Some(rules) = self.inner.get(&temperature) else {
            return if temperature > self.high {
                Safety::Danger
            } else {
                Safety::Ok
            };
        };
        rules
            .iter()
            .find(|r| r.humidity_range.contains(&humidity))
            .map_or(Safety::Ok, |r| r.level)
    }
}

#[expect(
    clippy::missing_panics_doc,
    reason = "panics only come from .expect()s"
)]
pub fn show_data(
    data: &SensorData,
    displays: &mut RpMax7219<'static>,
    safety_defs: &SafetyDefinitions,
    leds: &mut MultiLed,
) {
    leds.update(safety_defs, data.air_temp, data.humidity);

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

pub async fn collect(i2c: &mut RpI2C0Async, humid: Sht4x, temp: Sts4x, air: Sen5x) -> SensorData {
    let log_err = |err: &i2c::Error| {
        error!("[data] failed to measure: {:#b}", err);
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
    let mut req = client
        .request(Method::POST, query.as_str())
        .await?
        .headers(&[("token", include_str!("../config/backend_token"))]);

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
