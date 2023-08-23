use anyhow::{bail, Error, Result};
use libav::mathematics::rescale::Rescale;
use libav::util::rational::Rational;
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default, Hash)]
pub struct Timestamp(u64);
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default, Hash)]
pub struct Duration(u64);
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default, Hash)]
pub struct Timespan {
    start: Timestamp,
    end: Timestamp,
}

impl Timestamp {
    const TIMEBASE: Rational = Rational(1, 1000);
    pub const MIN: Timestamp = Self(u64::MIN);
    pub const MAX: Timestamp = Self(u64::MAX);

    pub fn from_libav_ts(ts: i64, time_base: Rational) -> Result<Self> {
        let ts = ts.rescale(time_base, Self::TIMEBASE);

        // av_rescale_rnd will return INT64_MIN if the result of the rescale is not representable
        //https://ffmpeg.org/doxygen/trunk/group__lavu__math.html#ga82d40664213508918093822461cc597e
        if ts == i64::MIN {
            bail!("Unrepresentable timestamp");
        } else if ts < 0 {
            bail!("Negative timestamp");
        } else {
            Ok(Self(ts.try_into().unwrap()))
        }
    }

    pub const fn from_secs(secs: u32) -> Self {
        Self(secs as u64 * 1000u64)
    }

    pub const fn as_millis(&self) -> u64 {
        self.0
    }

    pub const fn saturating_add(&self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration.as_millis()))
    }

    pub const fn saturating_sub(&self, duration: Duration) -> Self {
        Self(self.0.saturating_sub(duration.as_millis()))
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, d: Duration) -> Self::Output {
        Self(self.0 + d.as_millis())
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;

    fn sub(self, d: Duration) -> Self::Output {
        Self(self.0 - d.as_millis())
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

impl FromStr for Timestamp {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts[..] {
            [secs] => Ok(Timestamp::from_secs(secs.parse()?)),
            [mins, secs] => {
                let mins: u8 = mins.parse()?;
                let secs: u8 = secs.parse()?;
                Ok(Timestamp::from_secs(mins as u32 * 60 + secs as u32))
            }
            [hours, mins, secs] => {
                let hours: u8 = hours.parse()?;
                let mins: u8 = mins.parse()?;
                let secs: u8 = secs.parse()?; //TODO better errors
                Ok(Timestamp::from_secs(
                    60 * (hours as u32 * 60 + mins as u32) + secs as u32,
                ))
            }
            _ => Err(Error::msg("invalid timestamp")),
        }
    }
}

impl Duration {
    pub const fn from_millis(millis: u64) -> Duration {
        Self(millis)
    }

    pub const fn as_millis(&self) -> u64 {
        self.0
    }
}

impl Timespan {
    pub fn new(start: Timestamp, end: Timestamp) -> Self {
        Self {
            start: start.min(end),
            end: end.max(start),
        }
    }

    pub const fn start(&self) -> Timestamp {
        self.start
    }

    pub const fn end(&self) -> Timestamp {
        self.end
    }
}

impl From<Timespan> for (Timestamp, Timestamp) {
    fn from(span: Timespan) -> Self {
        (span.start(), span.end())
    }
}
