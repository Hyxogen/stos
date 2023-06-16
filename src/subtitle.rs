use anyhow::{Context, Error, Result};
use libav::{codec, decoder, media, codec::subtitle, codec::packet};
use libav::util::rational::Rational;
use log::{debug, trace};
use std::path::PathBuf;

pub enum Rect {
    Text(String),
}

pub struct Timestamp(i64);

impl Timestamp {
    pub fn from_timebase(ts: i64, time_base: Rational) -> Self {
        todo!()
    }
}

pub struct Subtitle {
    rects: Vec<Rect>,
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Subtitle {
    pub fn convert_subtitle(sub: &subtitle::Subtitle, packet: &packet::Packet, time_base: Rational) -> Result<Self> {
        let start = packet.pts().or(packet.dts()).ok_or(Error::msg("Subtitle is missing a timestamp"))?;
        let end = start + packet.duration();

        let start = Timestamp::from_timebase(start, time_base);
        let end = Timestamp::from_timebase(end, time_base);
        todo!()
    }
}

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &codec::packet::packet::Packet,
) -> Result<Option<subtitle::Subtitle>> {
    let mut subtitle = Default::default();
    match decoder.decode(packet, &mut subtitle).context("Failed to decode subtitle")? {
        true => Ok(Some(subtitle)),
        false => Ok(None),
    }
}

pub fn read_subtitles(file: &PathBuf, stream_idx: Option<usize>) -> Result<Vec<Subtitle>> {
    let file_str = file.to_string_lossy();

    let mut ictx = libav::format::input(file).context("Failed to open file")?;
    trace!("Opened {}", file_str);

    let sub_stream = if let Some(stream_idx) = stream_idx {
        ictx.streams().nth(stream_idx).ok_or_else(|| {
            Error::msg(format!(
                "The file does not have {} streams (has {})",
                stream_idx,
                ictx.nb_streams()
            ))
        })?
    } else {
        ictx.streams()
            .best(media::Type::Subtitle)
            .ok_or(Error::msg("The file does not have a subtitle stream"))?
    };

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
            subs.push(sub);
        }
    }
    debug!("{}: Read {} subtitle(s)", file_str, subs.len());

    Ok(subs)
}
