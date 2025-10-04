#![no_std]
#![no_main]

mod ble;
mod tasks;

use core::str::FromStr;

use cyw43::{JoinOptions, ScanOptions, bluetooth::BtDriver};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::{info, unwrap};
// use dpia::sensiron::{
//     sht4x::{Sht4x, model_addrs::SHT40_AD1B},
//     sts4x::{Sts4x, model_addrs::STS40_AD1B},
// };
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_net::{
    DhcpConfig, Stack, StackResources, StaticConfigV4,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_rp::{
    binary_info::{EntryAddr, rp_cargo_version, rp_program_build_attribute, rp_program_name},
    bind_interrupts,
    clocks::RoscRng,
    config::Config,
    gpio::{Level, Output},
    i2c,
    peripherals::{I2C0, I2C1, PIO0},
    pio::{self, Pio},
};
use embassy_time::Timer;
use reqwless::client::HttpClient;
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;

use crate::tasks::{bt, cyw43, net};

use {defmt_rtt as _, panic_probe as _};

#[used]
#[unsafe(link_section = ".bi_entries")]
pub static PICOTOOL_ENTRIES: [EntryAddr; 3] = [
    rp_program_name!(c"temperature"),
    rp_cargo_version!(),
    rp_program_build_attribute!(),
];

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    I2C0_IRQ   => i2c::InterruptHandler<I2C0>;
    I2C1_IRQ   => i2c::InterruptHandler<I2C1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Config::default());
    let mut rng = RoscRng;

    info!("Hello, World!");

    // let fw = include_bytes!("../../../cyw43-firmware/43439A0.bin");
    // let btfw = include_bytes!("../../../cyw43-firmware/43439A0_btfw.bin");
    // // "Country Locale Matrix"
    // let clm = include_bytes!("../../../cyw43-firmware/43439A0_clm.bin");
    // defmt::info!("fw={} btfw={} clm={}", fw.len(), btfw.len(), clm.len());

    // cyw43 firmware can be flashed with `just prepare-cyw43`
    let fw = unsafe { core::slice::from_raw_parts(0x101b0000 as *const u8, 231077) };
    let btfw = unsafe { core::slice::from_raw_parts(0x101f0000 as *const u8, 6164) };
    let clm = unsafe { core::slice::from_raw_parts(0x101f8000 as *const u8, 984) };

    // OP wireless power on signal
    let pwr = Output::new(p.PIN_23, Level::Low);
    // OP wireless SPI CS - when high also enables GPIO29 ADC pin to read VSYS
    let cs = Output::new(p.PIN_25, Level::Low);
    let mut pio0 = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio0.common,
        pio0.sm0,
        RM2_CLOCK_DIVIDER,
        pio0.irq0,
        cs,
        p.PIN_24, // OP/IP wireless SPI data/IRQ
        p.PIN_29, // OP/IP wireless SPI CLK/ADC mode (ADC3) to measure VSYS/3
        p.DMA_CH0,
    );

    info!("wifi pio and pins set up");

    // WIFI
    static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let cyw43_state = CYW43_STATE.init(cyw43::State::new());

    let (net_dev, bt_dev, mut control, runner) =
        cyw43::new_with_bluetooth(cyw43_state, pwr, spi, fw, btfw).await;
    unwrap!(spawner.spawn(cyw43(runner)));
    control.init(clm).await;

    info!("cyw43 set up");
    info!("scanning for wifi networks");

    let wanted_ssid = include_str!("../config/wanted_ssid");
    let ssid_pass = include_str!("../config/ssid_pass").as_bytes();

    {
        let mut scanner = control.scan(ScanOptions::default()).await;

        while let Some(scan) = scanner.next().await {
            let ssid = str::from_utf8(&scan.ssid).unwrap_or("???");
            info!(
                "found wifi: `{}`, strength {}dbM, channel {}",
                ssid, scan.rssi, scan.ctl_ch
            );

            if ssid == wanted_ssid {
                break;
            }
        }
    }

    info!("joining `{}`", wanted_ssid);
    for i in 0..=5 {
        if i == 5 {
            defmt::panic!("couldnt join wifi in 5 tries");
        }

        let join = control.join(wanted_ssid, JoinOptions::new(ssid_pass)).await;

        if join.is_ok() {
            break;
        }

        info!("retrying");
    }
    info!("joined successfully!");

    control.gpio_set(0, true).await;

    // NET
    static NET_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

    let mut dhcp_config = DhcpConfig::default();
    dhcp_config.hostname = Some(unwrap!(heapless_08::String::from_str("trevor's pico 2w"))); // TODO: change this probably
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    let seed = rng.next_u64();

    let (stack, runner) = embassy_net::new(
        net_dev,
        net_config,
        NET_RESOURCES.init(StackResources::new()),
        seed,
    );

    unwrap!(spawner.spawn(net(runner)));

    info!("waiting for dhcp");
    let net_config = wait_for_config(stack).await;
    info!("our ip is {:?}", net_config.address.address());

    static TCP_STATE: StaticCell<TcpClientState<1, 2, 1>> = StaticCell::new();
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    info!("tcp & dns set up");

    let q = dns
        .query("trevrosa.dev", embassy_net::dns::DnsQueryType::A)
        .await
        .unwrap();
    info!("trevrosa.dev: {:?}", q);

    let mut client = HttpClient::new(&tcp, &dns);

    // BLUETOOTH
    info!("starting bluetooth controller");
    let bt_control: ExternalController<BtDriver, 10> = ExternalController::new(bt_dev);
    let address = control.address().await;
    unwrap!(spawner.spawn(bt(bt_control, address)));

    // TODO: do we need two i2c buses?
    // let humidity = Sht4x::new(
    //     p.I2C0,
    //     p.PIN_1,
    //     p.PIN_0,
    //     Irqs,
    //     i2c::Config::default(),
    //     SHT40_AD1B,
    // );
    // let temp = Sts4x::new(
    //     p.I2C1,
    //     p.PIN_11,
    //     p.PIN_10,
    //     Irqs,
    //     i2c::Config::default(),
    //     STS40_AD1B,
    // );

    info!("finished!");

    let mut led = false;
    loop {
        control.gpio_set(0, led).await;
        Timer::after_secs(1).await;
        led = !led;
    }
}

async fn wait_for_config(stack: Stack<'static>) -> StaticConfigV4 {
    loop {
        if let Some(config) = stack.config_v4() {
            return config;
        }
        yield_now().await;
    }
}
