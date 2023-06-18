use crate::util::get_stream;
use anyhow::{Context, Error, Result};
use libav::mathematics::rescale::Rescale;
use libav::util::rational::Rational;
use libav::{codec, codec::packet, codec::subtitle, decoder, media};
use log::{debug, trace, warn};
use std::fmt;
use std::ops::Add;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Rect {
    Text(String),
}

impl From<subtitle::Rect<'_>> for Rect {
    fn from(rect: subtitle::Rect) -> Self {
        match rect {
            subtitle::Rect::Text(text) => Rect::Text(text.get().to_string()),
            subtitle::Rect::Ass(ass) => Rect::Text(ass.get().to_string()),
            subtitle::Rect::Bitmap(_) => Rect::Text("".to_string()),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Timestamp(i64);

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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Subtitle {
    rects: Vec<Rect>,
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Subtitle {
    pub fn convert_subtitle(
        sub: &subtitle::Subtitle,
        packet: &packet::Packet,
        time_base: Rational,
    ) -> Result<Self> {
        let start = packet
            .pts()
            .or(packet.dts())
            .ok_or(Error::msg("Subtitle is missing a timestamp"))?;
        let end = start + packet.duration();

        let start = Timestamp::from_timebase(start, time_base)
            .context("Failed to convert start timestamp")?
            + Timestamp::from_ms(sub.start());
        let end = Timestamp::from_timebase(end, time_base)
            .context("Failed to convert end timestamp")?
            + Timestamp::from_ms(sub.end());

        if start == end {
            Err(Error::msg("Subtitle is of zero length"))
        } else {
            if end < start {
                warn!("subtitle end is before start, will swap");
            }

            Ok(Self {
                start: start.min(end),
                end: end.max(start),
                rects: sub.rects().map(From::from).collect(),
            })
        }
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        !(self.end < other.start || other.end < self.start)
    }
}

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &codec::packet::packet::Packet,
) -> Result<Option<subtitle::Subtitle>> {
    let mut subtitle = Default::default();
    match decoder
        .decode(packet, &mut subtitle)
        .context("Failed to decode subtitle")?
    {
        true => Ok(Some(subtitle)),
        false => Ok(None),
    }
}

pub fn read_subtitles(file: &PathBuf, stream_idx: Option<usize>) -> Result<Vec<Subtitle>> {
    let file_str = file.to_string_lossy();

    let mut ictx = libav::format::input(file).context("Failed to open file")?;
    trace!("Opened {}", file_str);

    let sub_stream = get_stream(ictx.streams(), media::Type::Subtitle, stream_idx)
        .with_context(|| format!("{}: Failed to retrieve subtitle stream", file_str))?;

    let stream_index = sub_stream.index();
    let codec = sub_stream.parameters().id();

    debug!(
        "{}: Using subtitle stream at index {}. Codec type: {}",
        file_str,
        stream_index,
        codec.name()
    );

    let context = codec::context::Context::from_parameters(sub_stream.parameters())
        .with_context(|| format!("Failed to create codec context for {} codec", codec.name()))?;
    trace!(
        "{}: Created codec context for {} codec type",
        file_str,
        codec.name()
    );

    let mut decoder = context.decoder().subtitle().with_context(|| {
        format!(
            "{}: Failed to open decoder for {} codec type",
            file_str,
            codec.name()
        )
    })?;
    trace!("{}: Opened {} decoder", file_str, codec.name());

    let mut subs = Vec::new();

    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_index {
            continue;
        }

        if let Some(sub) = decode_subtitle(&mut decoder, &packet)? {
            match Subtitle::convert_subtitle(&sub, &packet, stream.time_base()) {
                Ok(sub) => {
                    subs.push(sub);
                }
                Err(err) => {
                    warn!("failed to convert subtitle: {}", err);
                }
            }
        }
    }
    if subs.is_empty() {
        warn!("{}: Contained no subtitles", file_str);
    } else {
        debug!("{}: Read {} subtitle(s)", file_str, subs.len());
    }

    Ok(subs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overlap() {
        let a = Subtitle {
            start: Timestamp(100),
            end: Timestamp(1000),
            rects: Default::default(),
        };
        let b = Subtitle {
            start: Timestamp(5000),
            end: Timestamp(5100),
            rects: Default::default(),
        };
        assert_eq!(a.overlaps(&b), false);
        assert_eq!(b.overlaps(&a), false);
    }

    #[test]
    fn partial_overlap() {
        let a = Subtitle {
            start: Timestamp(100),
            end: Timestamp(1000),
            rects: Default::default(),
        };
        let b = Subtitle {
            start: Timestamp(500),
            end: Timestamp(5000),
            rects: Default::default(),
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }

    #[test]
    fn exact_overlap() {
        let a = Subtitle {
            start: Timestamp(100),
            end: Timestamp(1000),
            rects: Default::default(),
        };
        let b = Subtitle {
            start: Timestamp(100),
            end: Timestamp(1000),
            rects: Default::default(),
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }

    #[test]
    fn complete_overlap() {
        let a = Subtitle {
            start: Timestamp(200),
            end: Timestamp(900),
            rects: Default::default(),
        };
        let b = Subtitle {
            start: Timestamp(100),
            end: Timestamp(1000),
            rects: Default::default(),
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }
}
