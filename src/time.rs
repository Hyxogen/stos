use anyhow::{bail, Result};
use libav::mathematics::rescale::Rescale;
use libav::util::rational::Rational;
use std::fmt;
use std::ops::Add;

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

    pub const fn as_millis(&self) -> u64 {
        self.0
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, d: Duration) -> Self::Output {
        Self(self.0 + d.as_millis())
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
