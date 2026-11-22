// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Hour-of-year to calendar conversion for a non-leap modelling year.

use std::fmt;

/// Three-letter month labels.
pub const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const DAYS_IN_MONTH: [usize; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Calendar position of an hour of the year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HourStamp {
    /// Month index, 0 = January.
    pub month: usize,
    /// Day of month, 1-based.
    pub day: usize,
    /// Hour of day, 0–23.
    pub hour: usize,
}

/// Convert hour-of-year `t` (0-based) to a calendar stamp.
pub fn hour_to_date(t: usize) -> HourStamp {
    let mut doy = t / 24;
    let mut m = 0;
    while m < 11 && doy >= DAYS_IN_MONTH[m] {
        doy -= DAYS_IN_MONTH[m];
        m += 1;
    }
    HourStamp { month: m, day: doy + 1, hour: t % 24 }
}

impl fmt::Display for HourStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:>2} {:02}:00", MONTHS[self.month], self.day, self.hour)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries() {
        assert_eq!(hour_to_date(0), HourStamp { month: 0, day: 1, hour: 0 });
        assert_eq!(hour_to_date(31 * 24), HourStamp { month: 1, day: 1, hour: 0 });
        assert_eq!(hour_to_date(8759), HourStamp { month: 11, day: 31, hour: 23 });
    }
}
