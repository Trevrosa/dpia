use cyw43::bluetooth::BtDriver;
use cyw43_pio::PioSpi;
use embassy_rp::{
    gpio::Output,
    peripherals::{DMA_CH0, PIO0},
};
use trouble_host::prelude::ExternalController;

use crate::ble::peripheral;

#[embassy_executor::task]
pub async fn cyw43(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn net(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn bt(controller: ExternalController<BtDriver<'static>, 10>, address: [u8; 6]) {
    peripheral(controller, address).await;
}
