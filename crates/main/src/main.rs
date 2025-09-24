#![no_std]
#![no_main]

mod ble;

use cyw43::{ScanOptions, bluetooth::BtDriver};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::unwrap;
use dpia::sensiron::{
    sht4x::{Sht4x, model_addrs::SHT40_AD1B},
    sts4x::{Sts4x, model_addrs::STS40_AD1B},
};
use embassy_executor::Spawner;
use embassy_rp::{
    binary_info::{EntryAddr, rp_cargo_version, rp_program_build_attribute, rp_program_name},
    bind_interrupts,
    config::Config,
    gpio::{Level, Output},
    i2c,
    peripherals::{DMA_CH0, I2C0, I2C1, PIO0},
    pio::{self, Pio},
};
use embassy_time::Timer;
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;

extern crate defmt_rtt;
extern crate panic_probe;

use crate::ble::peripheral;

#[used]
#[unsafe(link_section = ".bi_entries")]
pub static PICOTOOL_ENTRIES: [EntryAddr; 3] = [
    rp_program_name!(c"temperature"),
    rp_cargo_version!(),
    rp_program_build_attribute!(),
];

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) {
    runner.run().await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Config::default());

    defmt::info!("Hello, World!");

    let fw = include_bytes!("../../../cyw43-firmware/43439A0.bin");
    let btfw = include_bytes!("../../../cyw43-firmware/43439A0_btfw.bin");
    // "Country Locale Matrix"
    let clm = include_bytes!("../../../cyw43-firmware/43439A0_clm.bin");

    defmt::info!("fw={} btfw={} clm={}", fw.len(), btfw.len(), clm.len());

    // TODO: To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download ../../cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x10100000
    //     probe-rs download ../../cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

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

    defmt::debug!("pio and pins set up");

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());

    let (_net_dev, bt_dev, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));
    control.init(clm).await;

    defmt::debug!("cyw43 set up");
    defmt::info!("scanning for wifi now");

    {
        let mut scanner = control.scan(ScanOptions::default()).await;

        while let Some(scan) = scanner.next().await {
            let ssid = str::from_utf8(&scan.ssid).unwrap_or("???");
            defmt::info!(
                "found wifi: `{}`, strength {}dbM, channel {}",
                ssid,
                scan.rssi,
                scan.ctl_ch
            );
        }
    }

    // control.join("SSID", JoinOptions::new(b"PASSWORD")).await;

    defmt::info!("starting bluetooth controller");
    let bt_control: ExternalController<BtDriver, 10> = ExternalController::new(bt_dev);
    let address = control.address().await;
    peripheral(bt_control, address).await;

    let humidity = Sht4x::new(
        p.I2C0,
        p.PIN_1,
        p.PIN_0,
        Irqs,
        i2c::Config::default(),
        SHT40_AD1B,
    );
    let temp = Sts4x::new(
        p.I2C1,
        p.PIN_11,
        p.PIN_10,
        Irqs,
        i2c::Config::default(),
        STS40_AD1B,
    );

    loop {
        defmt::info!("finished");
        Timer::after_secs(1).await;
    }
}
