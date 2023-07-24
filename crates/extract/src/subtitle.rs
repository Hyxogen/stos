use crate::ass::DialogueEvent;
use crate::time::{Duration, Timestamp};
use anyhow::{Context, Error, Result};
use image::RgbaImage;
use libav::util::rational::Rational;
use libav::{codec, codec::packet, codec::subtitle, decoder, format::stream::Stream, media};
use log::{debug, trace, warn};
use std::path::PathBuf;
use std::slice;

use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Rect {
    Text(String),
    Ass(DialogueEvent),
    Bitmap(RgbaImage),
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

fn get_stream(
    mut streams: libav::format::context::common::StreamIter,
    medium: media::Type,
    stream_idx: Option<usize>,
) -> Result<Stream> {
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

fn bitmap_to_image(bitmap: &libav::codec::subtitle::Bitmap) -> Result<RgbaImage> {
    let colors = bitmap.colors();

    if colors <= 256 {
        let width: usize = bitmap
            .width()
            .try_into()
            .context("u32 does not fit in usize")?;
        let height: usize = bitmap
            .height()
            .try_into()
            .context("u32 does not fit in usize")?;

        let palette_linesize: usize = unsafe { (*bitmap.as_ptr()).linesize[0] }
            .try_into()
            .context("invalid palette linesize")?;
        let indices_linesize: usize = unsafe { (*bitmap.as_ptr()).linesize[0] }
            .try_into()
            .context("invalid indices linesize")?;

        let palette = unsafe {
            slice::from_raw_parts(
                (*bitmap.as_ptr()).data[1] as *mut u32,
                width * height * palette_linesize,
            )
        };

        let indices = unsafe {
            slice::from_raw_parts(
                (*bitmap.as_ptr()).data[0],
                width * height * indices_linesize,
            )
        };

        let mut image = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let index: usize = indices[y * indices_linesize + x]
                    .try_into()
                    .context("u32 does not fit in usize")?;
                let argb = palette[index].to_le_bytes();
                let a = argb[0];
                let r = argb[1];
                let g = argb[2];
                let b = argb[3];

                image.push(r);
                image.push(g);
                image.push(b);
                image.push(a);
            }
        }

        Ok(
            RgbaImage::from_raw(width.try_into().unwrap(), height.try_into().unwrap(), image)
                .ok_or(Error::msg("failed to convert bitmap image"))?,
        )
    } else {
        Err(Error::msg("unsupported bitmap format"))
    }
}

impl TryFrom<subtitle::Rect<'_>> for Rect {
    type Error = Error;

    fn try_from(rect: subtitle::Rect) -> Result<Self> {
        match rect {
            subtitle::Rect::Text(text) => Ok(Rect::Text(text.get().to_string())),
            subtitle::Rect::Ass(ass) => Ok(Rect::Ass(ass.try_into()?)),
            subtitle::Rect::Bitmap(bitmap) => Ok(Rect::Bitmap(bitmap_to_image(&bitmap)?)),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Subtitle {
    pub rects: Vec<Rect>,
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Subtitle {
    pub fn convert_subtitle(
        sub: &subtitle::Subtitle,
        packet: &packet::Packet,
        time_base: Rational,
    ) -> Result<Self> {
        let (start, end) = Self::get_span(sub, packet, time_base)?;

        if start != end {
            if end < start {
                warn!("subtitle end is before start, will swap");
            }
            let rects: Vec<Rect> = sub
                .rects()
                .map(TryFrom::try_from)
                .filter_map(|rect| match rect {
                    Ok(rect) => Some(rect),
                    Err(err) => {
                        warn!("failed to convert a rect: {}", err);
                        None
                    }
                })
                .collect();

            if rects.is_empty() {
                Err(Error::msg("No rects"))
            } else {
                Ok(Self {
                    start: start.min(end),
                    end: end.max(start),
                    rects,
                })
            }
        } else {
            Err(Error::msg("Subtitle is of zero length"))
        }
    }

    fn get_span(
        sub: &subtitle::Subtitle,
        packet: &packet::Packet,
        time_base: Rational,
    ) -> Result<(Timestamp, Timestamp)> {
        debug!("{:?}", packet.pts());
        let start = packet
            .pts()
            .or(packet.dts())
            .ok_or(Error::msg("Subtitle is missing a timestamp"))?;
        let end = start + packet.duration();

        let start = Timestamp::from_timebase(start, time_base)
            .context("Failed to convert start timestamp")?
            + Duration::from_ms(sub.start());
        let end = Timestamp::from_timebase(end, time_base)
            .context("Failed to convert end timestamp")?
            + Duration::from_ms(sub.end());
        Ok((start, end))
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
        .context("Failed to retrieve subtitle stream")?;

    let stream_index = sub_stream.index();
    let codec = sub_stream.parameters().id();

    debug!(
        "{}: Using {} subtitle stream at index {}",
        file_str,
        codec.name(),
        stream_index,
    );

    let context = codec::context::Context::from_parameters(sub_stream.parameters())
        .with_context(|| format!("Failed to create codec context for {} codec", codec.name()))?;
    trace!("{}: {}: Created codec context", file_str, codec.name());

    let mut decoder = context.decoder().subtitle().with_context(|| {
        format!(
            "{}: Failed to open decoder for {} codec type",
            file_str,
            codec.name()
        )
    })?;
    trace!("{}: {}: Opened decoder", file_str, codec.name());

    let mut subs = Vec::new();
    //TODO join duplicates for bitmap subtitles within Xms
    //Something with a hashmap of the latest subtitles?
    //Or better a btreemap of the latest image rects?

    let mut images: HashMap<RgbaImage, &mut Subtitle> = HashMap::new();

    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_index {
            continue;
        }

        if let Some(sub) = decode_subtitle(&mut decoder, &packet)? {
            match Subtitle::convert_subtitle(&sub, &packet, stream.time_base()) {
                Ok(sub) => {

                    for rect in &sub.rects {
                        if let Rect::Bitmap(image) = rect {
                            if let Some(prev_sub) = images.get_mut(image) {
                                prev_sub.end = sub.start;
                            }
                        }
                    }
                    //Perhaps better to have the subtitle data structure not include rects, so it's
                    //easier to "extend" their timestamps

                    subs.push(sub);
                }
                Err(err) => {
                    warn!("failed to convert subtitle: {}", err);
                }
            }
        }
    }
    Ok(subs)
}
