use crate::time::Timestamp;
use anyhow::Result;
use std::path::Path;

mod av {
    use crate::time::{Duration, Timestamp};
    use anyhow::{bail, Context, Error, Result};
    use libav::codec;
    use libav::codec::{decoder, packet::Packet, subtitle};
    use libav::format::context::{common::StreamIter, Input};
    use libav::format::stream::Stream;
    use libav::mathematics::rescale::Rescale;
    use libav::media;
    use libav::util::rational::Rational;
    use log::warn;
    use std::path::Path;

    pub(super) enum Rect {
        Text(String),
        Ass(String),
        Bitmap(String),
    }

    pub(super) struct Subtitle {
        start: Timestamp,
        end: Option<Timestamp>,
        pub(super) rects: Vec<Rect>,
    }

    struct AVSubtitle {
        subtitle: subtitle::Subtitle,
        start: Option<i64>,
        duration: i64,
    }

    impl TryFrom<subtitle::Rect<'_>> for Rect {
        type Error = Error;

        fn try_from(rect: subtitle::Rect) -> Result<Self> {
            match rect {
                subtitle::Rect::Text(text) => Ok(Rect::Text(text.get().to_string())),
                subtitle::Rect::Ass(text) => Ok(Rect::Ass(text.get().to_string())),//implement
                subtitle::Rect::Bitmap(_) => Ok(Rect::Bitmap("a".to_string())),//implement
                _ => todo!(),
            }
        }
    }

    impl TryFrom<AVSubtitle> for Subtitle {
        type Error = Error;

        fn try_from(av_sub: AVSubtitle) -> Result<Self> {
            let start = Timestamp::from_libav_ts(
                av_sub
                    .start
                    .ok_or(Error::msg("Subtitle packet is missing a timestamp"))?
                    .try_into()
                    .context("Subtitle packet has negative timestamp")?,
                AVSubtitle::TIMEBASE,
            )?;

            let duration = if av_sub.subtitle.end() > av_sub.subtitle.start()
                && av_sub.subtitle.end() != u32::MAX
            {
                Some(Duration::from_millis(
                    (av_sub.subtitle.end() - av_sub.subtitle.start()).into(),
                ))
            } else if av_sub.duration > 0 {
                //TODO check if packet.duration() is in millis or in timebase
                Some(Duration::from_millis(av_sub.duration.try_into().unwrap()))
            } else {
                None
            };

            let end = if let Some(duration) = duration {
                Some(start + duration)
            } else {
                None
            };

            let rects = av_sub
                .subtitle
                .rects()
                .map(TryFrom::try_from)
                .filter_map(|rect| match rect {
                    Ok(rect) => Some(rect),
                    Err(err) => {
                        warn!("failed to convert subtitle rect: {}", err);
                        None
                    }
                })
                .collect();

            Ok(Self { start, end, rects })
        }
    }

    impl Subtitle {
        pub(super) fn start(&self) -> Timestamp {
            self.start
        }

        pub(super) fn end(&self) -> Option<Timestamp> {
            self.end
        }
    }

    impl AVSubtitle {
        pub const TIMEBASE: Rational = Rational(1, 1000);

        fn decode(
            packet: Packet,
            decoder: &mut decoder::subtitle::Subtitle,
            timebase: Rational,
        ) -> Result<Option<Self>> {
            let mut subtitle = Default::default();
            match decoder
                .decode(&packet, &mut subtitle)
                .context("Failed to decode subtitle")?
            {
                true => Ok(Some(Self {
                    subtitle,
                    start: packet
                        .pts()
                        .or(packet.dts())
                        .map(|v| v.rescale(timebase, Self::TIMEBASE)),
                    duration: 0,
                })),
                false => Ok(None),
            }
        }
    }

    fn get_medium_name(medium: media::Type) -> &'static str {
        match medium {
            media::Type::Video => "video",
            media::Type::Audio => "audio",
            media::Type::Data => "data",
            media::Type::Subtitle => "subtitle",
            media::Type::Attachment => "attachment",
            _ => "unknown",
        }
    }

    fn get_stream(
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
            bail!("File does not have a `{}` stream", get_medium_name(medium));
        }
    }

    fn create_decoder(
        params: codec::parameters::Parameters,
    ) -> Result<decoder::subtitle::Subtitle> {
        let codec = params.id();
        let context = codec::context::Context::from_parameters(params).with_context(|| {
            format!(
                "Failed to create codec context for `{}` codec",
                codec.name()
            )
        })?;

        context
            .decoder()
            .subtitle()
            .with_context(|| format!("Failed to create decoder for `{}` codec", codec.name()))
    }

    fn read_subtitles_from_stream(
        mut ictx: Input,
        mut decoder: decoder::subtitle::Subtitle,
        stream_idx: usize,
    ) -> Result<Vec<Subtitle>> {
        let mut subs: Vec<Subtitle> = Vec::new();

        for (stream, packet) in ictx.packets() {
            if stream.index() != stream_idx {
                continue;
            }

            if let Some(av_sub) = AVSubtitle::decode(packet, &mut decoder, stream.time_base())? {
                match <AVSubtitle as TryInto<Subtitle>>::try_into(av_sub) {
                    Ok(sub) => {
                        if let Some(prev_sub) = subs.last_mut() {
                            if prev_sub.end.is_none() {
                                prev_sub.end = sub.end;
                            }
                        }

                        if !sub.rects.is_empty() {
                            subs.push(sub);
                        }
                    }
                    Err(err) => {
                        warn!("failed to convert subtitle: {}", err);
                    }
                }
            }
        }
        Ok(subs)
    }

    fn read_subtitles(ictx: Input, stream_idx: Option<usize>) -> Result<Vec<Subtitle>> {
        let stream = get_stream(ictx.streams(), media::Type::Subtitle, stream_idx)?;
        let stream_idx = stream.index();
        let decoder = create_decoder(stream.parameters())?;
        read_subtitles_from_stream(ictx, decoder, stream_idx)
    }

    pub(super) fn read_subtitles_from_file<P: AsRef<Path>>(
        file: &P,
        stream_idx: Option<usize>,
    ) -> Result<Vec<Subtitle>> {
        let ictx = libav::format::input(file).context("Failed to open file")?;
        //trace!("Opened {} for reading subtitles", file.to_string_lossy());

        read_subtitles(ictx, stream_idx)
    }
}

pub enum Dialogue {
    Text(String),
    Ass(String),
    Bitmap(String),
}

pub struct Subtitle {
    start: Timestamp,
    end: Timestamp,
    diag: Dialogue,
}

impl From<av::Rect> for Dialogue {
    fn from(rect: av::Rect) -> Self {
        match rect {
            av::Rect::Text(text) => Dialogue::Text(text),
            av::Rect::Ass(ass) => Dialogue::Ass(ass),
            av::Rect::Bitmap(image) => Dialogue::Bitmap(image),
        }
    }
}

impl Subtitle {
    fn convert(subtitle: av::Subtitle) -> impl Iterator<Item = Subtitle> {
        let start = subtitle.start();
        let end = subtitle.end();
        subtitle
            .rects
            .into_iter()
            .filter_map(move |rect| match end {
                Some(end) => Some(Self {
                    start,
                    end,
                    diag: rect.into(),
                }),
                None => None,
            })
    }

    pub fn start(&self) -> Timestamp {
        self.start
    }

    pub fn end(&self) -> Timestamp {
        self.end
    }

    pub fn dialogue(&self) -> &Dialogue {
        &self.diag
    }
}

pub fn read_subtitles_from_file<P: AsRef<Path>>(
    file: &P,
    stream_idx: Option<usize>,
) -> Result<impl Iterator<Item = Subtitle>> {
    let subs = av::read_subtitles_from_file(file, stream_idx)?;
    Ok(subs.into_iter().flat_map(Subtitle::convert))
}
