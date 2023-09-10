use anyhow::{bail, Result};
use libav::format::context::common::StreamIter;
use libav::format::stream::Stream;
use libav::media;

pub fn get_medium_name(medium: media::Type) -> &'static str {
    match medium {
        media::Type::Video => "video",
        media::Type::Audio => "audio",
        media::Type::Data => "data",
        media::Type::Subtitle => "subtitle",
        media::Type::Attachment => "attachment",
        _ => "unknown",
    }
}

pub fn get_stream(
    mut streams: StreamIter,
    medium: media::Type,
    stream_idx: Option<usize>,
) -> Result<Stream> {
    if let Some(stream_idx) = stream_idx {
        match streams.nth(stream_idx) {
            Some(stream) if stream.parameters().medium() == medium => Ok(stream),
            Some(stream) => bail!(
                "Stream at index {} is not a {} stream (is {} stream)",
                stream_idx,
                get_medium_name(medium),
                get_medium_name(stream.parameters().medium()),
            ),
            None => bail!("File does not have {} streams", stream_idx),
        }
    } else if let Some(stream) = streams.best(medium) {
        Ok(stream)
    } else {
        bail!("File does not have a {} stream", get_medium_name(medium));
    }
}
