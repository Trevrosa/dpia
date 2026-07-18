#![no_std]

use defmt::info;
use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use embassy_rp::{aon_timer::DateTime, spinlock_mutex::SpinlockRawMutex};
use embassy_sync::mutex::Mutex;
use heapless::{String, format};
use reqwless::{client::HttpClient, request::Method};

pub mod sensiron;

pub type HttpClientMutex = Mutex<
    SpinlockRawMutex<0>,
    HttpClient<'static, TcpClient<'static, 3, 2048, 2048>, DnsSocket<'static>>,
>;

/// use our api to get millis since unix epoch (corrected to our timezone)
pub async fn sync_epoch_ms(client: &'static HttpClientMutex) -> u64 {
    info!("syncing time");

    let client = &mut *client.lock().await;

    let mut rx_buf = [0u8; 1024];
    let mut req = client
        .request(Method::GET, "https://dpia.trevrosa.dev/time")
        .await
        .unwrap();

    let resp = req.send(&mut rx_buf).await.unwrap();
    info!("got {}", resp.status);

    let time = resp.body().read_to_end().await.unwrap();

    // api returns just a string
    let time = str::from_utf8(time)
        .expect("must be utf8")
        .parse()
        .expect("must be a number");

    info!("synced time is {}", time);

    time
}

/// saturday, midnight
pub fn next_weekend(mut now: DateTime) -> DateTime {
    let day_of_week = now.day_of_week as u8;

    assert!(matches!(day_of_week, 1..=5));

    // saturday is 6, today is monday-friday (1..=5), so 6-today is always positive
    // what if next weekend is in next month (day >= 31)? should be ok if no chrono
    now.day += 6 - day_of_week;

    now.hour = 0;
    now.minute = 0;
    now.second = 0;

    now
}

// max should be 35
pub fn debug_datetime(dt: &DateTime) -> String<40> {
    let timestamp = dt.timestamp_millis().expect("should be past the epoch");
    format!(
        "{}-{}-{} {:02}:{:02}:{:02} ({})",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second, timestamp
    )
    .unwrap()
}
