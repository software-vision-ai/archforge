//! Timestamps for the settings, without a calendar crate.

/// RFC 3339 in UTC, to the second, for seconds since 1970: the civil-from-days algorithm.
pub fn timestamp(unix_seconds: u64) -> String {
    let days = unix_seconds / 86_400;
    let secs = unix_seconds % 86_400;
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, secs % 3600 / 60, secs % 60)
}

/// The year, month and day of a day count since 1970-01-01, for days that are not negative.
fn civil(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_hold_at_the_epoch_a_leap_day_and_the_last_second_of_9999() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(timestamp(1_761_670_800), "2025-10-28T17:00:00Z");
        assert_eq!(timestamp(253_402_300_799), "9999-12-31T23:59:59Z");
    }
}
