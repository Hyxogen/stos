use anyhow::{Error, Result};
use libav::mathematics::rescale::Rescale;
use libav::util::rational::Rational;
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Timestamp(i64);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Duration(i64);

impl Duration {
    pub const fn from_ms(ms: u32) -> Self {
        Self(ms as i64 * 1000000)
    }

    pub const fn as_nanos(self) -> i64 {
        self.0
    }
}

impl Timestamp {
    const TIMEBASE: Rational = Rational(1, 1000);

    pub fn from_timebase(ts: i64, time_base: Rational) -> Result<Self> {
        let ts = ts.rescale(time_base, Self::TIMEBASE);

        if ts < 0 {
            Err(Error::msg("Timestamp is negative"))
        } else {
            Ok(Self(ts))
        }
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = self.as_millis();
        write!(
            f,
            "{}:{:02}:{:02}.{:03}",
            ts / (1000 * 60 * 60),
            (ts / (1000 * 60)) % 60,
            (ts / (1000)) % 60,
            ts % 1000
        )
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, duration: Duration) -> Self {
        Self(self.0 + duration.as_nanos())
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;

    fn sub(self, duration: Duration) -> Self {
        Self(self.0 - duration.as_nanos())
    }
}
