use crate::ass::DialogueEvent;
use crate::time::Timespan;
use anyhow::Result;
use image::RgbaImage;
use std::path::Path;

mod av {
    use crate::ass::DialogueEvent;
    use crate::time::{Duration, Timestamp};
    use crate::util::get_stream;
    use anyhow::{bail, Context, Error, Result};
    use image::RgbaImage;
    use libav::codec;
    use libav::codec::{decoder, packet::Packet, subtitle};
    use libav::format::context::Input;
    use libav::mathematics::rescale::Rescale;
    use libav::media;
    use libav::util::rational::Rational;
    use log::{trace, warn};
    use std::path::Path;

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub(super) enum Rect {
        Text(String),
        Ass(DialogueEvent),
        Bitmap(RgbaImage),
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
                subtitle::Rect::Ass(ass) => Ok(Rect::Ass(ass.try_into()?)),
                subtitle::Rect::Bitmap(bitmap) => Ok(Rect::Bitmap(bitmap_to_image(&bitmap)?)),
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
                    .ok_or(Error::msg("Subtitle packet is missing a timestamp"))?,
                AVSubtitle::TIMEBASE,
            )?;

            // from mpv source code (sub/sd_lavc.c)
            // libavformat sets duration==0, even if the duration is unknown. Some files
            // also have actually subtitle packets with duration explicitly set to 0
            // (yes, at least some of such mkv files were muxed by libavformat).
            // Assume there are no bitmap subs that actually use duration==0 for
            // hidden subtitle events.
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

            let end = duration.map(|duration| start + duration);

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
                    duration: packet.duration(),
                })),
                false => Ok(None),
            }
        }
    }

    fn bitmap_to_image(bitmap: &subtitle::Bitmap) -> Result<RgbaImage> {
        if bitmap.colors() <= 256 {
            let width: usize = bitmap
                .width()
                .try_into()
                .context("failed to convert u32 to usize")?;
            let height: usize = bitmap
                .height()
                .try_into()
                .context("failed to convert u32 to usize")?;

            // The bitmap is stored using a palette and an indices array into the palette.

            // There is a linesize[1] which seems like the one to use for the palette. But that
            // appears to be not the case. linesize[1] seems to be smaller than the indices allow
            // for. I've also looked at other code bases that decode bitmaps and they also only
            // seem to use linesize[0]
            let linesize: usize = unsafe { (*bitmap.as_ptr()).linesize[0] }
                .try_into()
                .context("invalid linesize")?;

            let palette = unsafe {
                std::slice::from_raw_parts(
                    (*bitmap.as_ptr()).data[1] as *mut u32,
                    width * height * linesize,
                )
            };

            let indices = unsafe {
                std::slice::from_raw_parts((*bitmap.as_ptr()).data[0], width * height * linesize)
            };

            let mut data = Vec::new();

            for y in 0..height {
                for x in 0..width {
                    let index: usize = indices[y * linesize + x]
                        .try_into()
                        .context("failed to convert u32 to usize")?;

                    let argb = palette[index].to_le_bytes();
                    let a = argb[0];
                    let r = argb[1];
                    let g = argb[2];
                    let b = argb[3];

                    data.push(r);
                    data.push(g);
                    data.push(b);
                    data.push(a);
                }
            }

            // These unwraps will not fail since in the begin we converted the width and height
            // from usize
            RgbaImage::from_raw(width.try_into().unwrap(), height.try_into().unwrap(), data)
                .ok_or(Error::msg("failed to convert bitmap image"))
        } else {
            bail!("Unsupported bitmap format");
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
                                prev_sub.end = Some(sub.start);
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
        trace!("Read {} subtitles", subs.len());
        Ok(subs)
    }

    fn read_subtitles(ictx: Input, stream_idx: Option<usize>) -> Result<Vec<Subtitle>> {
        let stream = get_stream(ictx.streams(), media::Type::Subtitle, stream_idx)?;
        let stream_idx = stream.index();
        trace!(
            "Using {} stream at index {}",
            stream.parameters().id().name(),
            stream_idx
        );

        let decoder = create_decoder(stream.parameters())?;
        trace!("Created {} decoder", stream.parameters().id().name());

        read_subtitles_from_stream(ictx, decoder, stream_idx)
    }

    pub(super) fn read_subtitles_from_file<P: AsRef<Path>>(
        file: &P,
        stream_idx: Option<usize>,
    ) -> Result<Vec<Subtitle>> {
        let file_str = file.as_ref().to_string_lossy();
        let ictx = libav::format::input(file).context("Failed to open file")?;
        trace!("Opened a {} for reading subtitles", file_str);

        read_subtitles(ictx, stream_idx)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Dialogue {
    Text(String),
    Ass(DialogueEvent),
    Bitmap(RgbaImage),
}

pub struct Subtitle {
    timespan: Timespan,
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
        subtitle.rects.into_iter().filter_map(move |rect| {
            end.map(|end| Self {
                timespan: Timespan::new(start, end),
                diag: rect.into(),
            })
        })
    }

    pub const fn timespan(&self) -> Timespan {
        self.timespan
    }

    pub fn set_timespan(&mut self, span: Timespan) -> &mut Self {
        self.timespan = span;
        self
    }

    pub fn dialogue(&self) -> &Dialogue {
        &self.diag
    }

    pub fn text(&self) -> Option<&str> {
        match self.dialogue() {
            Dialogue::Text(text) => Some(text),
            Dialogue::Ass(ass) => Some(&ass.text.dialogue),
            Dialogue::Bitmap(_) => None,
        }
    }
}

pub fn read_subtitles_from_file<P: AsRef<Path>>(
    file: &P,
    stream_idx: Option<usize>,
) -> Result<impl Iterator<Item = Subtitle>> {
    let subs = av::read_subtitles_from_file(file, stream_idx)?;
    Ok(subs.into_iter().flat_map(Subtitle::convert))
}
