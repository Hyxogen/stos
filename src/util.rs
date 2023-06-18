use anyhow::{Error, Result};
use libav::media;

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
