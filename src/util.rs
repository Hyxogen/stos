use anyhow::{Error, Result};
use libav::mathematics::rescale::Rescale;
use libav::media;
use libav::util::rational::Rational;
use std::fmt;
use std::ops::Add;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub const fn from_ms(ms: u32) -> Self {
        Self(ms as i64)
    }

    pub fn from_timebase(ts: i64, time_base: Rational) -> Result<Self> {
        let ts = ts.rescale(time_base, Self::time_base());

        if ts < 0 {
            Err(Error::msg("Timestamp is negative"))
        } else {
            Ok(Self(ts))
        }
    }

    pub const fn time_base() -> Rational {
        Rational(1, 1000000000)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = self.0.rescale(Self::time_base(), Rational::new(1, 1000000));
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

impl Add for Timestamp {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
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
