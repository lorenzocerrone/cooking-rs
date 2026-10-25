//! Calendar dates for the meal plan, with no dependencies.
//!
//! A [`Date`] is a day count since 1970-01-01 (proleptic Gregorian), so ordering
//! and day arithmetic are plain integer operations. On the wire it is an ISO
//! `YYYY-MM-DD` string. "Today" is never read here: the UI layer passes it in,
//! which keeps everything that depends on it host-testable.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A calendar day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(i32);

/// Three-letter weekday names, Monday first (index = [`Date::weekday`]).
pub const WEEKDAY_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Three-letter month names, January first.
pub const MONTH_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl Date {
    /// The date for a year, month (1–12) and day (1–31). `None` if it doesn't exist.
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return None;
        }
        // Howard Hinnant's days_from_civil.
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let m = month as i32;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i32 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Some(Date(era * 146_097 + doe - 719_468))
    }

    /// `(year, month, day)`.
    pub fn ymd(self) -> (i32, u32, u32) {
        // Howard Hinnant's civil_from_days.
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = yoe + era * 400 + i32::from(month <= 2);
        (year, month, day)
    }

    /// This date moved by `days` (negative goes back).
    pub fn add_days(self, days: i32) -> Self {
        Date(self.0 + days)
    }

    /// Whole days from `self` to `other` (positive when `other` is later).
    pub fn days_until(self, other: Date) -> i32 {
        other.0 - self.0
    }

    /// Day of the week, Monday = 0 .. Sunday = 6.
    pub fn weekday(self) -> usize {
        // 1970-01-01 was a Thursday (3).
        (self.0 + 3).rem_euclid(7) as usize
    }

    /// The first date on or after `self` that falls on `weekday` (Monday = 0).
    pub fn next_weekday(self, weekday: usize) -> Self {
        let ahead = (weekday as i32 - self.weekday() as i32).rem_euclid(7);
        self.add_days(ahead)
    }

    /// Parse an ISO `YYYY-MM-DD` date.
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('-');
        let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        Date::from_ymd(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Date::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid date `{s}`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::from_ymd(y, m, day).unwrap()
    }

    #[test]
    fn epoch_and_round_trips() {
        assert_eq!(d(1970, 1, 1), Date(0));
        assert_eq!(d(1969, 12, 31), Date(-1));
        for n in -800_000..800_000 {
            if n % 997 == 0 {
                let date = Date(n);
                let (y, m, day) = date.ymd();
                assert_eq!(Date::from_ymd(y, m, day), Some(date), "{n}");
            }
        }
    }

    #[test]
    fn rejects_impossible_dates() {
        assert!(Date::from_ymd(2026, 2, 29).is_none());
        assert!(Date::from_ymd(2028, 2, 29).is_some());
        assert!(Date::from_ymd(1900, 2, 29).is_none());
        assert!(Date::from_ymd(2000, 2, 29).is_some());
        assert!(Date::from_ymd(2026, 4, 31).is_none());
        assert!(Date::from_ymd(2026, 13, 1).is_none());
        assert!(Date::from_ymd(2026, 1, 0).is_none());
    }

    #[test]
    fn arithmetic_crosses_month_and_year_ends() {
        assert_eq!(d(2026, 10, 31).add_days(1), d(2026, 11, 1));
        assert_eq!(d(2026, 12, 31).add_days(1), d(2027, 1, 1));
        assert_eq!(d(2028, 2, 28).add_days(1), d(2028, 2, 29));
        assert_eq!(d(2026, 3, 1).add_days(-1), d(2026, 2, 28));
        assert_eq!(d(2026, 10, 25).days_until(d(2026, 11, 8)), 14);
    }

    #[test]
    fn weekdays() {
        assert_eq!(d(2026, 10, 25).weekday(), 6); // a Sunday
        assert_eq!(d(2026, 10, 26).weekday(), 0); // Monday
        assert_eq!(d(1970, 1, 1).weekday(), 3); // Thursday
        assert_eq!(d(1969, 12, 29).weekday(), 0);
        let sat = d(2026, 10, 24);
        assert_eq!(sat.next_weekday(5), sat, "same weekday is today");
        assert_eq!(sat.next_weekday(0), d(2026, 10, 26));
        assert_eq!(sat.next_weekday(4), d(2026, 10, 30));
    }

    #[test]
    fn iso_text_and_serde() {
        let date = d(2026, 3, 7);
        assert_eq!(date.to_string(), "2026-03-07");
        assert_eq!(Date::parse("2026-03-07"), Some(date));
        for bad in [
            "2026-3-7",
            "2026-02-30",
            "26-03-07",
            "2026-03-07-1",
            "nope",
            "",
        ] {
            assert_eq!(Date::parse(bad), None, "{bad}");
        }
        assert_eq!(serde_json::to_string(&date).unwrap(), "\"2026-03-07\"");
        assert_eq!(
            serde_json::from_str::<Date>("\"2026-03-07\"").unwrap(),
            date
        );
        assert!(serde_json::from_str::<Date>("\"2026-13-01\"").is_err());
    }
}
