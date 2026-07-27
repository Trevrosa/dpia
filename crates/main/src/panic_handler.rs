use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicBool, Ordering},
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    static PANICKED: AtomicBool = AtomicBool::new(false);

    cortex_m::interrupt::disable();

    // Guard against infinite recursion, just in case.
    if !PANICKED.load(Ordering::Relaxed) {
        PANICKED.store(true, Ordering::Relaxed);

        defmt::error!("{}", defmt::Display2Format(info));
    }

    cortex_m::peripheral::SCB::sys_reset();
}
