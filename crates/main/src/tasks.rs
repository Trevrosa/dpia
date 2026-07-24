use cyw43_pio::PioSpi;
use defmt::{error, info, warn};
use dpia::{
    HttpClientMutex, debug_datetime, ep0159, next_weekend,
    sensiron::{sen5x::Sen5x, sht4x::Sht4x, sts4x::Sts4x},
    sync_epoch_ms, try_forever,
};
use embassy_rp::{
    Peri,
    aon_timer::{self, DayOfWeek},
    gpio::Output,
    i2c::{self, I2c},
    peripherals::{DMA_CH1, I2C0, PIN_13, PIO0, POWMAN, UART0},
    uart,
};
use embassy_time::{Duration, Timer};
use heapless::String;
use max7219::MAX7219;

use crate::{
    GlobalSensorDataMutex, Irqs,
    data::{collect, show_data, submit},
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

#[embassy_executor::task]
pub async fn power_manager(powman: Peri<'static, POWMAN>, client: &'static HttpClientMutex) -> ! {
    let mut timer = aon_timer::AonTimer::new(
        powman,
        Irqs,
        aon_timer::Config {
            clock_source: aon_timer::ClockSource::Lposc,
            clock_freq_khz: 32,
            alarm_wake_mode: aon_timer::AlarmWakeMode::Both,
        },
    );

    info!("[pwr] aon timer set up");

    // timer starts stopped
    timer.set_counter(try_forever(|| sync_epoch_ms(client), Duration::from_secs(2)).await);
    timer.start();

    // if we start on a weekday, wait until the weekend to start sleeping, else start immediately
    let now = timer.now_as_datetime().expect("year should be valid");
    info!("[pwr] it is now {}", debug_datetime(&now));
    if matches!(now.day_of_week as u8, 1..=5) {
        info!("[pwr] it's a weekday, waiting for saturday to sleep");

        let weekend = next_weekend(now);

        info!("[pwr] waiting for {}", debug_datetime(&weekend));
        timer
            .set_alarm_at_datetime(weekend)
            .expect("dt should be in the future");
        timer.wait_for_alarm().await;
    }

    info!("[pwr] it's a weekend, sleeping now");

    loop {
        let now = timer.now_as_datetime().expect("year should be valid");
        let days = if now.day_of_week == DayOfWeek::Saturday {
            2
        } else {
            1
        };

        // it should now be saturday 00:00, sleep until next monday 6:00
        timer
            .set_alarm_after(Duration::from_secs((days * 60 * 60 * 24) + (6 * 60 * 60)))
            .unwrap();

        #[cfg(feature = "sleep")]
        info!("[pwr] pretending to sleep");

        #[cfg(not(feature = "sleep"))]
        {
            info!("[pwr] sleeping soon");
            Timer::after_secs(3).await;
            info!("[pwr] sleeping now");
            embassy_rp::clocks::dormant_sleep();
        }

        // it should now be monday 6:00, wait until saturday 00:00
        info!("[pwr] woke up, syncing time");
        timer.stop();
        timer.set_counter(try_forever(|| sync_epoch_ms(client), Duration::from_secs(2)).await);
        timer.start();

        let now = timer.now_as_datetime().expect("year should be valid");
        info!("[pwr] it is now {}", debug_datetime(&now));
        let weekend = next_weekend(now);

        info!("[pwr] waiting for {}", debug_datetime(&weekend));
        timer
            .set_alarm_at_datetime(weekend)
            .expect("dt should be in future");
        timer.wait_for_alarm().await;
    }
}

pub type RpMax7219<'a> =
    MAX7219<max7219::connectors::PinConnector<Output<'a>, Output<'a>, Output<'a>>>;
pub type RpI2C0Async = I2c<'static, I2C0, i2c::Async>;

#[embassy_executor::task]
pub async fn data_collector(
    mut i2c: RpI2C0Async,
    humid: Sht4x,
    temp: Sts4x,
    air: Sen5x,
    mut displays: Option<RpMax7219<'static>>,
    client: &'static HttpClientMutex,
    global_data: &'static GlobalSensorDataMutex,
) -> ! {
    let mut do_everything = async || {
        info!("collecting");
        let data = collect(&mut i2c, humid, temp, air).await;
        info!("got data: {:?}", data);
        {
            let global_data = &mut *global_data.lock().await;
            *global_data = data;
        }
        if let Err(err) = submit(client, &data).await {
            error!("failed to submit: {}", err);
        }
        if let Some(ref mut displays) = displays {
            show_data(&data, displays);
        }
    };

    do_everything().await;

    loop {
        Timer::after_secs(30).await;
        do_everything().await;
    }
}

#[cfg(feature = "bt")]
use cyw43::bluetooth::BtDriver;
#[cfg(feature = "bt")]
use trouble_host::prelude::ExternalController;

#[cfg(feature = "bt")]
#[embassy_executor::task]
pub async fn ble(
    controller: ExternalController<BtDriver<'static>, 10>,
    address: [u8; 6],
    global: &'static GlobalSensorDataMutex,
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
        crate::bt::server_loop(stack.peripheral, &server, global),
    )
    .await;
}

// https://wiki.52pi.com/index.php?title=EP-0159
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

    loop {
        let mut byte = [0; 1];
        if let Err(err) = uart.read(&mut byte).await {
            warn!("[ep0159] failed to read: {}", err);
            continue;
        }

        let ready = ep0159::handle_byte(byte[0], &mut buf, &mut values, &mut pipes);
        if ready {
            values.log();
        }
    }
}
