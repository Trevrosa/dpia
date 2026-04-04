#![no_std]

use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use reqwless::client::HttpClient;

pub mod sensiron;

/// use timeapi.io to get the current unix epoch in ms
pub async fn sync_epoch_ms(
    client: &mut HttpClient<'_, TcpClient<'_, 3, 2048, 2048>, DnsSocket<'_>>,
) -> u64 {
    let mut cur_timestamp = [0u8; 35];
    client
        .request(
            reqwless::request::Method::GET,
            "https://timeapi.io/api/v1/time/current/unix_ms",
        )
        .await
        .unwrap()
        .send(&mut cur_timestamp)
        .await
        .unwrap();

    str::from_utf8(&cur_timestamp)
        .unwrap()
        .split(':')
        .nth(1)
        .unwrap()[..13] // unix ms is 13 digits
        .parse::<u64>()
        .unwrap()
}
