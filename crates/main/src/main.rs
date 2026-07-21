#![no_std]
#![no_main]

mod bt;
pub mod data;
mod tasks;

use core::str::FromStr;

use cyw43::bluetooth::BtDriver;
use cyw43::{A4, Aligned, JoinOptions, ScanOptions};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::{error, info, unwrap};
use dpia::HttpClientMutex;
use dpia::sensiron::{
    sen5x::{self, Sen5x},
    sht4x::{Sht4x, model_addrs::SHT40_AD1B},
    sts4x::{Sts4x, model_addrs::STS40_CD1B},
};
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_net::{
    DhcpConfig, Stack, StackResources, StaticConfigV4,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_rp::spinlock_mutex::SpinlockRawMutex;
use embassy_rp::{
    aon_timer,
    binary_info::{EntryAddr, rp_cargo_version, rp_program_build_attribute, rp_program_name},
    bind_interrupts,
    clocks::RoscRng,
    config::Config,
    dma,
    gpio::{Level, Output},
    i2c::{self, I2c},
    peripherals::{DMA_CH0, DMA_CH1, I2C0, I2C1, PIO0},
    pio::{self, Pio},
};
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use max7219::MAX7219;
use reqwless::client::{HttpClient, TlsConfig};
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;

use crate::data::SensorData;
use crate::tasks::{ble, cyw43, data_collector, net, power_manager};

use {defmt_rtt as _, panic_probe as _};

#[used]
#[unsafe(link_section = ".bi_entries")]
pub static PICOTOOL_ENTRIES: [EntryAddr; 3] = [
    rp_program_name!(c"temperature"),
    rp_cargo_version!(),
    rp_program_build_attribute!(),
];

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0       => pio::InterruptHandler<PIO0>;
    DMA_IRQ_0        => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
    I2C0_IRQ         => i2c::InterruptHandler<I2C0>;
    I2C1_IRQ         => i2c::InterruptHandler<I2C1>;
    POWMAN_IRQ_TIMER => aon_timer::InterruptHandler;
});

pub type GlobalSensorDataMutex = Mutex<SpinlockRawMutex<1>, SensorData>;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Config::default());
    let mut rng = RoscRng;

    info!("Hello, World!!");

    // let fw = aligned_bytes!("../../../cyw43-firmware/43439A0.bin");
    // let btfw = aligned_bytes!("../../../cyw43-firmware/43439A0_btfw.bin");
    // let clm = aligned_bytes!("../../../cyw43-firmware/43439A0_clm.bin");
    // let nvram = aligned_bytes!("../../../cyw43-firmware/nvram_rp2040.bin");
    // defmt::info!("fw={} btfw={} clm={} nvram={}", fw.len(), btfw.len(), clm.len(), nvram.len());

    // cyw43 firmware can be flashed with `just prepare-cyw43`
    let fw: Aligned<A4, _> = unsafe { Aligned(*(0x101b_0000 as *const [u8; 231_077])) };
    let btfw: Aligned<A4, _> = unsafe { Aligned(*(0x101f_0000 as *const [u8; 6164])) };
    // "Country Locale Matrix"
    let clm = unsafe { core::slice::from_raw_parts(0x101f_2000 as *const u8, 984) };
    let nvram: Aligned<A4, _> = unsafe { Aligned(*(0x101f_4000 as *const [u8; 743])) };

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
        dma::Channel::new(p.DMA_CH0, Irqs),
    );

    info!("wifi pio and pins set up");

    // WIFI
    static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let cyw43_state = CYW43_STATE.init(cyw43::State::new());

    let (net_dev, bt_dev, mut control, runner) =
        cyw43::new_with_bluetooth(cyw43_state, pwr, spi, &fw, &btfw, &nvram).await;
    spawner.spawn(unwrap!(cyw43(runner)));
    control.init(clm).await;

    info!("cyw43 set up");
    info!("scanning for wifi networks");

    let wanted_ssid = include_str!("../config/wanted_ssid");
    let ssid_pass = include_str!("../config/ssid_pass");

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

        let join = control
            .join(wanted_ssid, JoinOptions::new(ssid_pass.as_bytes()))
            .await;

        if let Err(err) = join {
            error!("failed to join: {}", err);
        } else {
            break;
        }

        info!("retrying");
    }
    info!("joined successfully!");

    control.gpio_set(0, true).await;

    // NET
    static NET_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

    let mut dhcp_config = DhcpConfig::default();
    dhcp_config.hostname = Some(unwrap!(heapless::String::from_str("trevor's pico 2w"))); // TODO: change this probably
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    let seed = rng.next_u64();

    let (stack, runner) = embassy_net::new(
        net_dev,
        net_config,
        NET_RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(unwrap!(net(runner)));

    info!("waiting for dhcp");
    let net_config = wait_for_config(stack).await;
    info!("our ip is {:?}", net_config.address.address());

    static TCP_STATE: StaticCell<TcpClientState<3, 2048, 2048>> = StaticCell::new();
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    info!("tcp & dns set up");

    let q = dns
        .query("trevrosa.dev", embassy_net::dns::DnsQueryType::A)
        .await
        .unwrap();
    info!("trevrosa.dev: {:?}", q);

    // TLS
    static TLS_R: StaticCell<[u8; 16640]> = StaticCell::new();
    static TLS_W: StaticCell<[u8; 16640]> = StaticCell::new();
    #[allow(clippy::large_stack_arrays)]
    let tls_r = TLS_R.init_with(|| [0; _]);
    #[allow(clippy::large_stack_arrays)]
    let tls_w = TLS_W.init_with(|| [0; _]);

    // no certificate verification but should be ok since we only request some domains
    let tls_config = TlsConfig::new(seed, tls_r, tls_w, reqwless::client::TlsVerify::None);

    static TCP: StaticCell<TcpClient<'static, 3, 2048, 2048>> = StaticCell::new();
    static DNS: StaticCell<DnsSocket<'static>> = StaticCell::new();
    static CLIENT: StaticCell<HttpClientMutex> = StaticCell::new();
    let client = CLIENT.init(Mutex::new(HttpClient::new_with_tls(
        TCP.init(tcp),
        DNS.init(dns),
        tls_config,
    )));
    info!("http client set up");

    // BLUETOOTH
    static GLOBAL_SENSOR_DATA: StaticCell<GlobalSensorDataMutex> = StaticCell::new();
    let global_sensor_data = GLOBAL_SENSOR_DATA.init(Mutex::new(SensorData::default()));
    info!("starting bluetooth controller");
    let bt_control: ExternalController<BtDriver, 10> = ExternalController::new(bt_dev);
    let address = control.address().await;
    spawner.spawn(unwrap!(ble(bt_control, address, global_sensor_data)));

    // SENSORS
    let i2c_config = i2c::Config::default();
    let mut i2c: I2c<'_, I2C0, i2c::Async> =
        I2c::new_async(p.I2C0, p.PIN_17, p.PIN_16, Irqs, i2c_config);
    defmt::info!("initialised i2c bus!");

    let humidity = Sht4x::new(SHT40_AD1B);
    let temp = Sts4x::new(STS40_CD1B);
    let air = Sen5x::new(sen5x::ADDR);

    let air_serial = air.serial_num(&mut i2c).await.unwrap_or_default();
    let air_serial = str::from_utf8(&air_serial).unwrap_or("???");

    defmt::info!(
        "sht: {}, sts: {}, sen: {}",
        humidity.serial_num(&mut i2c).await,
        temp.serial_num(&mut i2c).await,
        air_serial
    );

    // DISPLAY
    let sck = Output::new(p.PIN_2, Level::Low);
    let mosi = Output::new(p.PIN_3, Level::Low);
    let cs = Output::new(p.PIN_4, Level::High);
    let displays = MAX7219::from_pins(2, mosi, cs, sck).ok();
    if displays.is_none() {
        error!("failed to init max7219");
    }

    // DATA COLLECTION
    spawner.spawn(unwrap!(data_collector(
        i2c,
        humidity,
        temp,
        air,
        displays,
        client,
        global_sensor_data
    )));

    // POWER MANAGEMENT
    spawner.spawn(unwrap!(power_manager(p.POWMAN, client)));

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
