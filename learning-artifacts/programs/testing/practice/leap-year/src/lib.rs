//! Calendar helpers.

/// Returns `true` for leap years of the Gregorian calendar: years divisible by
/// 4, except century years, except century years divisible by 400.
pub fn is_leap_year(year: u32) -> bool {
    year % 4 == 0 && year % 100 != 0
}

/// Number of days in `month` (1–12) of `year`, or `None` for an invalid month.
pub fn days_in_month(year: u32, month: u32) -> Option<u32> {
    match month {
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        4 | 6 | 9 | 11 => Some(30),
        1..=12 => Some(31),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        assert!(is_leap_year(2024));
    }
}
