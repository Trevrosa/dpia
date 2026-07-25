use bytemuck::{NoUninit, bytes_of};
use crc::{CRC_32_CKSUM, Crc};
use defmt::warn;
use embassy_rp::{
    Peri,
    flash::{self, Flash},
    peripherals::FLASH,
};

#[derive(NoUninit, Clone, Copy)]
#[repr(C, align(4))]
pub struct Data {
    pub boot_count: u32,
    pub wanted_ssid: [u8; 32], // max ssid length
    pub ssid_pass: [u8; 64],   // 63 is max for wpa2
    crc32: u32,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            boot_count: 0,
            wanted_ssid: [0; _],
            ssid_pass: [0; _],
            crc32: 0,
        }
    }
}

impl defmt::Format for Data {
    fn format(&self, fmt: defmt::Formatter) {
        let wanted_ssid = str::from_utf8(&self.wanted_ssid).unwrap_or("INVALID");
        let ssid_pass = str::from_utf8(&self.ssid_pass).unwrap_or("INVALID");
        defmt::write!(
            fmt,
            r#"Data {{ boot_count: {}, wanted_ssid: "{}", ssid_pass: "{}", crc32: {:#010x} }}"#,
            self.boot_count,
            wanted_ssid,
            ssid_pass,
            self.crc32
        );
    }
}

unsafe extern "C" {
    static __persistent_flash_start: [u8; 0];
    static __persistent_flash_end: [u8; 0];
}

pub fn crc32() -> Crc<u32> {
    Crc::<u32>::new(&CRC_32_CKSUM)
}

const ADDR_OFFSET: u32 = 0x1000_0000;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

impl Data {
    fn checksum(&self) -> u32 {
        // the crc32 is stored at the end of the struct, and the struct is repr(C)
        let bytes = &bytes_of(self)[..size_of::<Data>() - 4];
        crc32().checksum(bytes)
    }

    /// Reads from the section and checks its crc32. If it does not match, returns an empty [`Data`].
    pub fn read() -> Self {
        let flash_ptr: *const Data = (&raw const __persistent_flash_start).cast();
        let data = unsafe { *flash_ptr };

        let sum = data.checksum();
        if sum == data.crc32 {
            data
        } else {
            warn!(
                "crc did not match (ours: {:#010x} != {:#010x})",
                sum, data.crc32
            );
            Data::default()
        }
    }

    /// Calculates the crc32 of `self` and writes it into the flash section.
    ///
    /// # Errors
    ///
    /// Fails if the flash could not be written to successfully.
    pub fn write(&self, flash: Peri<'_, FLASH>) -> Result<(), flash::Error> {
        let mut data = *self;
        data.crc32 = self.checksum();

        let start_offset = &raw const __persistent_flash_start as u32 - ADDR_OFFSET;
        let end_offset = &raw const __persistent_flash_end as u32 - ADDR_OFFSET;

        let mut flash: Flash<'_, _, _, FLASH_SIZE> = Flash::new_blocking(flash);
        flash.blocking_erase(start_offset, end_offset)?;
        flash.blocking_write(start_offset, bytes_of(&data))
    }
}
