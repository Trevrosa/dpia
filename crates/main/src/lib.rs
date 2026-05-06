#![no_std]

use defmt::info;
use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use reqwless::{client::HttpClient, request::Method};

pub mod sensiron;

/// use timeapi.io to get the current unix epoch in ms
pub async fn sync_epoch_ms(
    client: &mut HttpClient<'_, TcpClient<'_, 3, 2048, 2048>, DnsSocket<'_>>,
) -> u64 {
    info!("syncing time");

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

    info!("time is {}", time);

    time
}
