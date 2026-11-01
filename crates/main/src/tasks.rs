use cyw43_pio::PioSpi;
use defmt::{error, info, warn};
use dpia::{
    HttpClientMutex, ep0159,
    sensiron::{sen5x::Sen5x, sht4x::Sht4x, sts4x::Sts4x},
    try_limited,
};
use embassy_rp::{
    Peri,
    gpio::{Level, Output, Pin},
    i2c::{self, I2c},
    peripherals::{DMA_CH1, I2C0, PIN_13, PIO0, POWMAN, UART0},
    uart,
};
use embassy_sync::once_lock::OnceLock;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use max7219::MAX7219;

use crate::{
    Irqs, SensorDataMutex,
    data::{Safety, SafetyDefinitions, collect, show_data, submit},
};

#[embassy_executor::task]
pub async fn cyw43(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn net(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[cfg(feature = "powman")]
#[embassy_executor::task]
pub async fn power_manager(powman: Peri<'static, POWMAN>, client: &'static HttpClientMutex) -> ! {
    use dpia::time::{debug_datetime, is_weekend, next_day, next_monday};
    use dpia::{power::dormant_sleep, sync_epoch_ms};
    use embassy_rp::aon_timer::{self, DayOfWeek};

    let mut timer = aon_timer::AonTimer::new(
        powman,
        Irqs,
        aon_timer::Config {
            clock_source: aon_timer::ClockSource::Lposc,
            clock_freq_khz: 32,
            alarm_wake_mode: aon_timer::AlarmWakeMode::Both,
        },
    );
    let try_sync = async |client: &'static HttpClientMutex| -> u64 {
        try_limited(|| sync_epoch_ms(client), Duration::from_secs(2), 15, "sync").await
    };

    info!("[pwr] aon timer set up");

    info!("[pwr] syncing time");
    // timer starts stopped
    timer.set_counter(try_sync(client).await);
    timer.start();

    // if we start on a weekend, sleep until the next week
    let now = timer.now_as_datetime().expect("year should be valid");

    if is_weekend(&now) {
        info!("[pwr] it is now {}", debug_datetime(&now));
        let monday = next_monday(now, 7).timestamp_millis().unwrap();
        info!(
            "[pwr] it is a weekend, sleeping to next monday 7am ({})",
            monday
        );
        timer.set_alarm(monday).expect("should not be in the past");
        dormant_sleep();
    }

    // it should be a weekday.
    // if before 3pm, wait for 3pm.
    // if after 3pm, sleep to next day 7 am.
    // if after 3pm AND if friday sync time and then sleep to next monday.
    loop {
        // it should now be a weekday, check time
        let now = timer.now_as_datetime().expect("year should be valid");
        let current_secs =
            (now.hour as u64 * 60 * 60) + (now.minute as u64 * 60) + (now.second as u64);

        info!("[pwr] it is now {}", debug_datetime(&now));

        let target_secs = if now.hour > 14 {
            // it is after 3pm, if today is friday, sync time then sleep to next monday, otherwise sleep until next day 7 am.
            info!("[pwr] it is after 3pm");
            if now.day_of_week == DayOfWeek::Friday {
                info!("[pwr] it is friday, syncing time then sleeping to next monday 7am");
                timer.stop();
                timer.set_counter(try_sync(client).await);
                timer.start();

                let target = next_monday(now, 7).timestamp_millis().unwrap();
                info!("[pwr] which is {}", target);
                timer.set_alarm(target).expect("not in past");
            } else {
                let target = next_day(now, 7).timestamp_millis().unwrap();
                info!("[pwr] sleeping to tomorrow 7am ({})", target);
                timer.set_alarm(target).expect("not in past");
            }
            dormant_sleep();

            continue;
        } else {
            // it is before 3pm, wait until 3:00 pm
            15 * 60 * 60
        };

        let sleep_time = target_secs - current_secs;
        info!("[pwr] waiting for 3pm (in {} secs)", sleep_time);
        Timer::after_secs(sleep_time).await;
    }
}

pub type RpMax7219<'a> =
    MAX7219<max7219::connectors::PinConnector<Output<'a>, Output<'a>, Output<'a>>>;
pub type RpI2C0Async = I2c<'static, I2C0, i2c::Async>;

pub struct MultiLed<'a> {
    ok: Output<'a>,
    caution: Output<'a>,
    danger: Output<'a>,
}

impl<'a> MultiLed<'a> {
    pub fn new(
        ok: Peri<'a, impl Pin>,
        caution: Peri<'a, impl Pin>,
        danger: Peri<'a, impl Pin>,
    ) -> Self {
        Self {
            ok: Output::new(ok, Level::Low),
            caution: Output::new(caution, Level::Low),
            danger: Output::new(danger, Level::Low),
        }
    }

    pub fn update(
        &mut self,
        safety_defs: &SafetyDefinitions,
        temperature: Option<f32>,
        humidity: Option<u8>,
    ) {
        self.ok.set_low();
        self.caution.set_low();
        self.danger.set_low();

        let (Some(temperature), Some(humidity)) = (temperature, humidity) else {
            warn!("[safety] temp/humidity unavailable");
            return;
        };

        let led = match safety_defs.safety_of(temperature, humidity) {
            Safety::Ok => &mut self.ok,
            Safety::Caution => &mut self.caution,
            Safety::Danger => &mut self.danger,
        };

        led.set_high();
    }
}

pub struct Sensors {
    pub humidity: Sht4x,
    pub temp: Sts4x,
    pub air: Sen5x,
}

#[embassy_executor::task]
pub async fn data_collector(
    mut i2c: RpI2C0Async,
    sensors: Sensors,
    mut displays: Option<RpMax7219<'static>>,
    mut leds: MultiLed<'static>,
    client: &'static HttpClientMutex,
    global_data: &'static SensorDataMutex,
    safety_defs: SafetyDefinitions,
) -> ! {
    const COLLECTION_INTERVAL: Duration = Duration::from_secs(60);
    const SUCCESSIVE_SUBMIT_FAIL_LIMIT: u32 = 5;
    let mut submit_fails = 0;

    let Sensors {
        humidity,
        temp,
        air,
    } = sensors;

    let mut do_everything = async || {
        if submit_fails > SUCCESSIVE_SUBMIT_FAIL_LIMIT {
            defmt::panic!(
                "failed to submit too many ({}) times",
                SUCCESSIVE_SUBMIT_FAIL_LIMIT
            );
        }

        info!("collecting");
        let data = collect(&mut i2c, humidity, temp, air).await;
        info!("got data: {:?}", data);
        {
            let global_data = &mut *global_data.lock().await;
            *global_data = data;
        }
        if let Err(err) = submit(client, &data).await {
            submit_fails += 1;
            error!("failed to submit: {}", err);
        } else {
            submit_fails = 0;
        }
        if let Some(ref mut displays) = displays {
            show_data(&data, displays, &safety_defs, &mut leds);
        }
    };

    do_everything().await;

    loop {
        Timer::after(COLLECTION_INTERVAL).await;
        do_everything().await;
    }
}

#[cfg(feature = "bt")]
use cyw43::bluetooth::BtDriver;
#[cfg(feature = "bt")]
use trouble_host::prelude::ExternalController;

#[derive(defmt::Format)]
pub struct WifiCredentials {
    pub ssid: String<64>,
    pub pass: String<64>,
}

impl WifiCredentials {
    /// Returns (ssid, pass) as `&str`s.
    pub fn as_strs(&self) -> (&str, &str) {
        (self.ssid.as_str(), self.pass.as_str())
    }
}

#[cfg(feature = "bt")]
#[embassy_executor::task]
pub async fn ble(
    controller: ExternalController<BtDriver<'static>, 10>,
    address: [u8; 6],
    global_data: &'static SensorDataMutex,
    wifi: &'static OnceLock<WifiCredentials>,
) {
    use crate::bt;
    use embassy_futures::join::join;
    use trouble_host::{
        Address, HostResources,
        gap::{GapConfig, PeripheralConfig},
        prelude::{DefaultPacketPool, appearance},
    };

    const CONNECTIONS: usize = 1;
    const L2CAP_CHANNELS: usize = 2; // signalling + att
    const ADV_SETS: usize = 1;

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS, L2CAP_CHANNELS, ADV_SETS> =
        HostResources::new();
    let address = Address::random(address);
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let stack = stack.build();

    info!("starting advertising and GATT service");

    let server = bt::Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "dpia",
        appearance: &appearance::sensor::MULTI_SENSOR,
    }))
    .unwrap();

    join(
        crate::bt::ble_task(stack.runner),
        crate::bt::server_loop(stack.peripheral, &server, global_data, wifi),
    )
    .await;
}

// https://wiki.52pi.com/index.php?title=EP-0159
#[cfg(feature = "monitor-bat")]
#[embassy_executor::task]
pub async fn power_status(
    uart: Peri<'static, UART0>,
    rx: Peri<'static, PIN_13>,
    rx_dma: Peri<'static, DMA_CH1>,
) {
    let mut uart = uart::UartRx::new(uart, rx, Irqs, rx_dma, uart::Config::default());

    let mut buf: String<50> = String::new();
    let mut pipes: u8 = 0;
    let mut values = ep0159::Values::default();

    let start = Instant::now();
    loop {
        let mut byte = [0; 1];
        if let Err(err) = uart.read(&mut byte).await {
            warn!("[ep0159] failed to read: {}", err);
            continue;
        }

        let ready = ep0159::handle_byte(byte[0], &mut buf, &mut values, &mut pipes);
        if ready && start.elapsed().as_secs().is_multiple_of(5) {
            values.log();
        }
    }
}
