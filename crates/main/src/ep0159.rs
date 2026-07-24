use defmt::{info, warn};
use heapless::string::StringView;

#[derive(Default)]
pub struct Values {
    /// in: mV, store: V
    bat_voltage: f32,
    /// mA
    current_use: f32,
    // in: mA * 10, store: mA
    charging_in: f32,
    unknown: f32,
}

impl Values {
    pub fn log(&self) {
        info!(
            "bat: {} V, consuming: {}mA, charging: {}mA",
            self.bat_voltage, self.current_use, self.charging_in
        );
    }
}

/// `buf` should have a capacity of at least 50.
/// 
/// Returns `true` if all values are updated (on the fourth pipe).
/// 
/// # Panics
/// 
/// Fails if `buf` runs out of capacity. 
pub fn handle_byte(byte: u8, buf: &mut StringView, values: &mut Values, pipes: &mut u8) -> bool {
    if byte > 0x7f || byte == b'\r' {
        return false;
    }

    if byte == b'|' {
        *pipes += 1;

        let ready = parse(buf, values, pipes);
        if ready.is_none() {
            warn!("[ep0159] failed to parse value");
        }
        buf.clear();
        ready.is_some_and(|r| r)
    } else {
        buf.push(byte as char).unwrap();
        false
    }
}

/// Returns [`None`] if a value failed to be parsed from `buf`.
pub fn parse(buf: &str, values: &mut Values, pipes: &mut u8) -> Option<bool> {
    match pipes {
        1 => values.bat_voltage = buf.parse::<f32>().ok()? / 1000.0,
        2 => values.current_use = buf.parse().ok()?,
        3 => values.charging_in = buf.parse::<f32>().ok()? / 10.0,
        4 => {
            *pipes = 0;
            values.unknown = buf.parse().ok()?;
            return Some(true);
        }
        _ => unreachable!(),
    }
    Some(false)
}
