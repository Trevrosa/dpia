#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::binary_info::{rp_cargo_version, rp_program_build_attribute, rp_program_name, EntryAddr};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

#[used]
#[unsafe(link_section = ".bi_entries")]
pub static PICOTOOL_ENTRIES: [EntryAddr; 3] = [
    rp_program_name!(c"temperature"),
    rp_cargo_version!(),
    rp_program_build_attribute!()
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    let periphs = embassy_rp::init(Default::default());

    defmt::info!("Hello, World!");

    loop {
        Timer::after(Duration::from_millis(100)).await;
    }
}
