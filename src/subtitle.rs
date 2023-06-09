extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::{packet, subtitle};
use ffmpeg::mathematics::rescale::Rescale;
use ffmpeg::util::rational::Rational;

const ONE_BILLIONTH: Rational = Rational(1, 1000000000);

pub enum SubtitleError {
    NegativeStart,
    NegativeEnd,
    MissingTimestamp,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Subtitle {
    rects: Vec<Rect>,
    pub start: i64,
    pub end: i64,
}

impl Extend<Rect> for Subtitle {
    fn extend<T: IntoIterator<Item = Rect>>(&mut self, iter: T) {
        self.rects.extend(iter);
    }
}

impl Subtitle {
    pub fn new(
        subtitle: &subtitle::Subtitle,
        packet: &packet::Packet,
        time_base: Rational,
    ) -> Result<Self, SubtitleError> {
        let start = packet
            .pts()
            .or(packet.dts())
            .ok_or(SubtitleError::MissingTimestamp)?
            + Into::<i64>::into(subtitle.start()).rescale(ONE_BILLIONTH, time_base);

        let end = start
            + packet.duration()
            + Into::<i64>::into(subtitle.end()).rescale(ONE_BILLIONTH, time_base);

        Ok(Self {
            start: start.try_into().map_err(|_| SubtitleError::NegativeStart)?,
            end: end.try_into().map_err(|_| SubtitleError::NegativeEnd)?,
            rects: subtitle.rects().map(From::from).collect(),
        })
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        return !(self.end < other.start
            || other.end < self.start
            || other.start > self.end
            || self.start > other.end);
    }
}

pub struct SubtitleList {
    subs: Vec<Subtitle>,
    pub time_base: Rational,
}

impl SubtitleList {
    pub fn new(time_base: Rational) -> Self {
        Self {
            subs: Vec::default(),
            time_base,
        }
    }

    pub fn add_sub(&mut self, sub: Subtitle) -> &mut Self {
        self.subs.push(sub);
        self
    }

    pub fn subs(&self) -> impl Iterator<Item = &Subtitle> {
        self.subs.iter()
    }

    pub fn coalesce(self) -> Self {
        let mut subs = Vec::new();

        for subtitle in self.subs {
            if subs.is_empty() {
                subs.push(subtitle);
            } else {
                let last_idx = subs.len() - 1;
                let prev_subtitle = &mut subs[last_idx];

                if !prev_subtitle.overlaps(&subtitle) {
                    subs.push(subtitle);
                } else {
                    prev_subtitle.start = std::cmp::min(prev_subtitle.start, subtitle.start);
                    prev_subtitle.end = std::cmp::max(prev_subtitle.end, subtitle.end);
                    prev_subtitle.extend(subtitle.rects);
                }
            }
        }

        Self {
            subs,
            time_base: self.time_base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overlap() {
        let a = Subtitle {
            start: 100,
            end: 1000,
            ..Default::default()
        };
        let b = Subtitle {
            start: 5000,
            end: 5100,
            ..Default::default()
        };
        assert_eq!(a.overlaps(&b), false);
        assert_eq!(b.overlaps(&a), false);
    }

    #[test]
    fn partial_overlap() {
        let a = Subtitle {
            start: 100,
            end: 1000,
            ..Default::default()
        };
        let b = Subtitle {
            start: 500,
            end: 5000,
            ..Default::default()
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }

    #[test]
    fn exact_overlap() {
        let a = Subtitle {
            start: 100,
            end: 1000,
            ..Default::default()
        };
        let b = Subtitle {
            start: 100,
            end: 1000,
            ..Default::default()
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }

    #[test]
    fn complete_overlap() {
        let a = Subtitle {
            start: 200,
            end: 900,
            ..Default::default()
        };
        let b = Subtitle {
            start: 100,
            end: 1000,
            ..Default::default()
        };
        assert_eq!(a.overlaps(&b), true);
        assert_eq!(b.overlaps(&a), true);
    }
}
