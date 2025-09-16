#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::{
    binary_info::{rp_cargo_version, rp_program_build_attribute, rp_program_name, EntryAddr},
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::PIO0,
    pio::{self, Pio}, spi::Spi,
};
use embassy_time::{Duration, Timer};
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
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Hello, World!");

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let btfw = include_bytes!("../cyw43-firmware/43439A0_btfw.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

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

    loop {
        Timer::after_secs(1).await
    }
}
