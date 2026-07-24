use bytemuck::{NoUninit, bytes_of};
use crc::{CRC_32_CKSUM, Crc};
use embassy_rp::{
    Peri,
    flash::{self, Flash},
    peripherals::FLASH,
};

#[derive(defmt::Format, NoUninit, Clone, Copy)]
#[repr(C, align(4))]
pub struct Data {
    pub boot_count: u32,
    pub wanted_ssid: [u8; 32], // max ssid length
    pub ssid_pass: [u8; 64],   // 63 is max for wpa2
    crc32: u32,
}

unsafe extern "C" {
    static __persistent_flash_start: [u8; 0];
    static __persistent_flash_end: [u8; 0];
}

const ADDR_OFFSET: usize = 0x10000;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

impl Data {
    pub fn read() -> &'static Self {
        let flash_ptr: *const Data = (&raw const __persistent_flash_start).cast();
        unsafe { &*flash_ptr }
    }

    /// Calculates the crc32 of `self` and writes it into the flash section.
    ///
    /// # Errors
    ///
    /// Fails if the flash could not be written to successfully.
    pub fn write(&self, flash: Peri<'_, FLASH>) -> Result<(), flash::Error> {
        let mut data = *self;
        data.crc32 = 0;
        let crc = Crc::<u32>::new(&CRC_32_CKSUM);
        data.crc32 = crc.checksum(bytes_of(&data));

        let mut flash: Flash<'_, _, _, FLASH_SIZE> = Flash::new_blocking(flash);
        flash.blocking_write(
            ADDR_OFFSET as u32 + &raw const __persistent_flash_start as u32,
            bytemuck::bytes_of(&data),
        )
    }
}
