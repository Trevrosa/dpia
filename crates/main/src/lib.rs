#![no_std]

use core::fmt::Write;

use defmt::info;
use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use embassy_rp::{aon_timer::DateTime, spinlock_mutex::SpinlockRawMutex};
use embassy_sync::mutex::Mutex;
use heapless::{
    LenType, String, format,
    string::{StringInner, StringStorage},
};
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

pub fn fmt_f32_for_display(f: f32) -> (String<8>, u8, usize) {
    debug_assert!(f <= 100.0);

    let mut s = format!(8; "{f:.1}").unwrap();

    let len_before = s.len();

    // len_before should be <= 5 and must be <=8
    pad(&mut s, ' ', 8 - len_before);

    // there must be 1 decimal point. including the dot, minus 2
    // 3 <= len_before <= 8, (min of 0.0, max defined by capacity)
    // if in=100.0, len=5
    // dist = len - 3 = 2
    // 7 - dist = 5
    // write from the left, so set the 5th bit to 1:
    // 0b0010_0000
    let dots = 1 << 7 - (len_before - 3);

    (s, dots, len_before)
}

pub fn pad_u8_for_display(u: u8, after: Option<usize>) -> String<8> {
    let mut s = String::new();

    if let Some(index) = after {
        // theoretically could have been 100.0 with len=5
        debug_assert!(index <= 5);
        pad(&mut s, ' ', index);
    }

    debug_assert!(u <= 100);
    // max +len of 3
    write!(&mut s, "{u}").unwrap();

    s
}

pub fn pad<LenT, S>(s: &mut StringInner<LenT, S>, char: char, num: usize)
where
    LenT: LenType,
    S: StringStorage,
{
    debug_assert!(s.len() + num <= s.capacity());

    for _ in 0..num {
        s.push(char).unwrap();
    }
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
