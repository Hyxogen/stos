use anyhow::{bail, Result};
use libav::format::context::common::StreamIter;
use libav::format::stream::Stream;
use libav::media;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum StreamSelector<'a> {
    Index(usize),
    Language(&'a str),
    Best,
}

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

pub fn get_stream<'a>(
    mut streams: StreamIter<'a>,
    medium: media::Type,
    selector: StreamSelector<'_>,
) -> Result<Stream<'a>> {
    match selector {
        StreamSelector::Index(stream_idx) => match streams.nth(stream_idx) {
            Some(stream) if stream.parameters().medium() == medium => Ok(stream),
            Some(stream) => bail!(
                "Stream at index {} is not a {} stream (is {} stream)",
                stream_idx,
                get_medium_name(medium),
                get_medium_name(stream.parameters().medium()),
            ),
            None => bail!("File does not have {} streams", stream_idx),
        },
        StreamSelector::Language(lang) => {
            for stream in streams {
                if stream.parameters().medium() == medium {
                    if let Some(stream_lang) = stream.metadata().get("language") {
                        if stream_lang.eq_ignore_ascii_case(lang) {
                            return Ok(stream);
                        }
                    }
                }
            }
            bail!(
                "File does not have a {} language {} stream",
                lang,
                get_medium_name(medium)
            )
        }
        StreamSelector::Best => {
            if let Some(stream) = streams.best(medium) {
                Ok(stream)
            } else {
                bail!("File does not have a {} stream", get_medium_name(medium))
            }
        }
    }
}
