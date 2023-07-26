use anyhow::{Error, Result};
use libav::mathematics::rescale::Rescale;
use libav::media;
use libav::util::rational::Rational;
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

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
    pub const MIN: Timestamp = Self(0);
    pub const MAX: Timestamp = Self(i64::MAX);
    const TIMEBASE: Rational = Rational(1, 1000000000);

    #[cfg(test)]
    pub const fn from_ms(ms: u32) -> Self {
        Self(ms as i64 * 1000000)
    }

    pub const fn from_secs(ms: u32) -> Self {
        Self(ms as i64 * 1000000000)
    }

    pub fn from_timebase(ts: i64, time_base: Rational) -> Result<Self> {
        let ts = ts.rescale(time_base, Self::TIMEBASE);

        if ts < 0 {
            Err(Error::msg("Timestamp is negative"))
        } else {
            Ok(Self(ts))
        }
    }

    pub fn checked_add(&self, duration: Duration) -> Option<Timestamp> {
        if let Some(val) = self.0.checked_add(duration.as_nanos()) {
            if val >= 0 {
                Some(Self(val))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn checked_sub(&self, duration: Duration) -> Option<Timestamp> {
        if let Some(val) = self.0.checked_sub(duration.as_nanos()) {
            if val >= 0 {
                Some(Self(val))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = self.0.rescale(Self::TIMEBASE, Rational::new(1, 1000000));
        write!(
            f,
            "{}:{:02}:{:02}.{:03}",
            ts / (1000 * 1000 * 60 * 60),
            (ts / (1000 * 1000 * 60)) % 60,
            (ts / (1000 * 1000)) % 60,
            (ts / 1000) % 1000
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

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for media::Type {
    fn name(&self) -> &'static str {
        match self {
            media::Type::Video => "video",
            media::Type::Audio => "audio",
            media::Type::Data => "data",
            media::Type::Subtitle => "subtitle",
            media::Type::Attachment => "attachment",
            _ => "unknown",
        }
    }
}

pub fn get_stream(
    mut streams: libav::format::context::common::StreamIter,
    medium: media::Type,
    stream_idx: Option<usize>,
) -> Result<libav::format::stream::Stream> {
    match stream_idx {
        Some(stream_idx) => match streams.nth(stream_idx) {
            Some(stream) if stream.parameters().medium() == medium => Ok(stream),
            Some(stream) => Err(Error::msg(format!(
                "Stream at index {} is not a {} stream (is {} stream)",
                stream_idx,
                medium.name(),
                stream.parameters().medium().name()
            ))),
            None => Err(Error::msg(format!(
                "File does not have a {} streams",
                stream_idx
            ))),
        },
        None => Ok(streams
            .best(medium)
            .ok_or_else(|| Error::msg(format!("File does not have a {} stream", medium.name())))?),
    }
}
