use embassy_rp::aon_timer::{DateTime, DayOfWeek};
use heapless::{String, format};

#[inline]
pub fn is_weekend(dt: &DateTime) -> bool {
    dt.day_of_week == DayOfWeek::Saturday || dt.day_of_week == DayOfWeek::Sunday
}

#[inline]
pub fn is_weekday(dt: &DateTime) -> bool {
    matches!(dt.day_of_week as u8, 1..=5)
}

pub fn next_day(mut now: DateTime, hour: u8) -> DateTime {
    now.day += 1;
    now.hour = hour;
    now.minute = 0;
    now.second = 0;

    now
}

pub fn next_monday(mut now: DateTime, hour: u8) -> DateTime {
    match now.day_of_week {
        DayOfWeek::Saturday => now.day += 2,
        DayOfWeek::Sunday => now.day += 1,
        // weekday is 1..=5, 8-weekday is always positive
        // monday is 1, so 8-1 is 7
        weekday => now.day += 8 - weekday as u8,
    }

    now.hour = hour;
    now.minute = 0;
    now.second = 0;

    now
}

/// saturday, midnight
///
/// # Panics
///
/// Fails if the `now` is in the weekend.
pub fn next_weekend(mut now: DateTime) -> DateTime {
    let day_of_week = now.day_of_week as u8;

    assert!(is_weekday(&now));

    // saturday is 6, today is monday-friday (1..=5), so 6-today is always positive
    // what if next weekend is in next month (day >= 31)? should be ok if no chrono
    now.day += 6 - day_of_week;

    now.hour = 0;
    now.minute = 0;
    now.second = 0;

    now
}

// max should be 35

#[expect(
    clippy::missing_panics_doc,
    reason = "dt should reasonably be past the unix epoch"
)]
pub fn debug_datetime(dt: &DateTime) -> String<40> {
    let timestamp = dt.timestamp_millis().expect("should be past the epoch");
    format!(
        "{}-{}-{} {:02}:{:02}:{:02} ({})",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second, timestamp
    )
    .unwrap()
}
