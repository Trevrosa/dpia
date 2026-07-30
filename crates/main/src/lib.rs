#![no_std]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::items_after_statements)]

use core::num::ParseIntError;

use defmt::{error, info, panic, warn};
use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use embassy_rp::spinlock_mutex::SpinlockRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use reqwless::{client::HttpClient, request::Method, response::StatusCode};

pub mod ep0159;
pub mod power;
pub mod sensiron;
pub mod time;

pub type HttpClientMutex = Mutex<
    SpinlockRawMutex<0>,
    HttpClient<'static, TcpClient<'static, 3, 2048, 2048>, DnsSocket<'static>>,
>;

/// Run a future that returns a [`Result`] until it returns [`Ok`] with a `delay` between tries.
///
/// # Panics
///
/// Panics if we reach `max_tries`.
pub async fn try_limited<T, E>(
    mut f: impl AsyncFnMut() -> Result<T, E>,
    delay: Duration,
    max_tries: usize,
    task_name: &'static str,
) -> T
where
    E: defmt::Format,
{
    let mut i = 1;
    loop {
        if i == max_tries {
            panic!("[{}] reached max tries ({})", task_name, max_tries);
        }

        match f().await {
            Ok(res) => break res,
            Err(err) => error!("[{}] failed: {}", task_name, err),
        }

        Timer::after(delay).await;

        i += 1;
        warn!("[{}] try #{}", task_name, i);
    }
}

#[derive(defmt::Format)]
pub enum SyncError {
    Reqwless(reqwless::Error),
    HttpError(StatusCode),
    ParseError,
}

impl From<reqwless::Error> for SyncError {
    fn from(value: reqwless::Error) -> Self {
        Self::Reqwless(value)
    }
}

impl From<ParseIntError> for SyncError {
    fn from(_value: ParseIntError) -> Self {
        Self::ParseError
    }
}

/// use our api to get millis since unix epoch (corrected to our timezone)
///
/// # Errors
///
/// Fails if the request failed to be sent, if the response status was not successful, or if the response failed to be parsed as an int.
#[expect(clippy::missing_panics_doc, reason = "panics are only from .expect()s")]
pub async fn sync_epoch_ms(client: &'static HttpClientMutex) -> Result<u64, SyncError> {
    info!("[sync] syncing time");

    let client = &mut *client.lock().await;

    let mut rx_buf = [0u8; 1024];
    let mut req = client
        .request(Method::GET, "https://dpia.trevrosa.dev/time")
        .await?;

    let resp = req.send(&mut rx_buf).await?;
    info!("[sync] got {}", resp.status);

    if !resp.status.is_successful() {
        return Err(SyncError::HttpError(resp.status));
    }

    let time = resp.body().read_to_end().await?;

    // api returns just a string
    let time = str::from_utf8(time).expect("must be utf8").parse()?;

    info!("[sync] synced time is {}", time);

    Ok(time)
}
