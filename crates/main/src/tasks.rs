use cyw43_pio::PioSpi;
use defmt::{error, info};
use dpia::{
    HttpClientMutex, debug_datetime, next_weekend,
    sensiron::{sen5x::Sen5x, sht4x::Sht4x, sts4x::Sts4x},
    sync_epoch_ms,
};
use embassy_rp::{
    Peri,
    aon_timer::{self, DayOfWeek},
    clocks::dormant_sleep,
    gpio::Output,
    i2c::{self, I2c},
    peripherals::{I2C0, PIO0, POWMAN},
};
use embassy_time::{Duration, Timer};
use max7219::MAX7219;

use crate::{
    Irqs,
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

    info!("aon timer set up");

    // timer starts stopped
    timer.set_counter(sync_epoch_ms(client).await);
    timer.start();

    // if we start on a weekday, wait until the weekend to start sleeping, else start immediately
    let now = timer.now_as_datetime().expect("year should be valid");
    info!("it is now {}", debug_datetime(&now));
    if matches!(now.day_of_week as u8, 1..=5) {
        info!("it's a weekday, waiting for saturday to sleep");

        let weekend = next_weekend(now);

        info!("waiting for {}", debug_datetime(&weekend));
        timer
            .set_alarm_at_datetime(weekend)
            .expect("dt should be in the future");
        timer.wait_for_alarm().await;
    }

    info!("it's a weekend, sleeping now");

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

        info!("sleeping soon");
        Timer::after_secs(3).await;
        info!("sleeping now");
        dormant_sleep();

        // it should now be monday 6:00, wait until saturday 00:00
        info!("woke up, syncing time");
        timer.stop();
        timer.set_counter(sync_epoch_ms(client).await);
        timer.start();

        let now = timer.now_as_datetime().expect("year should be valid");
        info!("it is now {}", debug_datetime(&now));
        let weekend = next_weekend(now);

        info!("waiting for {}", debug_datetime(&weekend));
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
    mut displays: RpMax7219<'static>,
    client: &'static HttpClientMutex,
) -> ! {
    let mut do_everything = async || {
        let data = collect(&mut i2c, humid, temp, air).await;
        if let Err(err) = submit(client, &data).await {
            error!("failed to submit: {}", err);
        }
        show_data(&data, &mut displays);
    };

    do_everything().await;

    loop {
        Timer::after_secs(30).await;
        do_everything().await;
    }
}

// #[embassy_executor::task]
// pub async fn bt(controller: ExternalController<BtDriver<'static>, 10>, address: [u8; 6]) {
//     peripheral(controller, address).await;
// }
