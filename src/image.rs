use crate::time::Timestamp;
use crate::util::get_stream;
use anyhow::{bail, Context, Result};
use crossbeam_channel::{Receiver, Sender};
pub use image::{DynamicImage, ImageBuffer, RgbImage, Rgba};
use indicatif::ProgressBar;
use libav::codec;
use libav::codec::decoder;
use libav::format::context::Input;
use libav::media;
use libav::software::scaling;
use libav::util::frame;
use log::{trace, warn};
use std::path::Path;

fn extract_images_from_stream<'a, I>(
    sender: Sender<(String, DynamicImage)>,
    mut ictx: Input,
    mut decoder: decoder::video::Video,
    mut scaler: scaling::context::Context,
    points: I,
    stream_idx: usize,
    pb: ProgressBar,
) -> Result<()>
where
    I: Iterator<Item = (Timestamp, &'a str)>,
{
    let mut points = points.peekable();

    //This unwrap will never fail, since the stream_idx was checked before in
    //extract_images_from_file
    let time_base = ictx.streams().nth(stream_idx).unwrap().time_base();

    let mut receive_and_process_frame = |decoder: &mut decoder::video::Video| -> Result<bool> {
        let mut decoded = frame::video::Video::empty();

        while decoder.receive_frame(&mut decoded).is_ok() {
            let frame_ts = Timestamp::from_libav_ts(decoded.pts().unwrap_or(0), time_base)?;

            if let Some((ts, _)) = points.peek() {
                if frame_ts < *ts {
                    continue;
                }

                let mut rgb_frame = frame::video::Video::empty();
                scaler
                    .run(&decoded, &mut rgb_frame)
                    .context("Failed to scale frame")?;

                if let Some(image) = RgbImage::from_raw(
                    rgb_frame.width(),
                    rgb_frame.height(),
                    rgb_frame.data(0).to_vec(),
                ) {
                    while let Some((_, name)) = points.next_if(|(ts, _)| frame_ts >= *ts) {
                        pb.inc(1);
                        sender
                            .send((name.to_string(), image.clone().into()))
                            .context("Failed to send image")?;
                    }
                } else {
                    bail!("Failed to convert frame to image");
                }
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_idx {
            decoder
                .send_packet(&packet)
                .context("Failed to send packet to decoder")?;

            if !receive_and_process_frame(&mut decoder)? {
                break;
            }
        }
    }

    decoder
        .send_eof()
        .context("Failed to send EOF to decoder")?;
    receive_and_process_frame(&mut decoder)?;

    let remaining = points.count();
    if remaining > 0 {
        warn!("was not able to extract last {} images", remaining);
    }
    Ok(())
}
fn create_decoder(params: codec::parameters::Parameters) -> Result<decoder::video::Video> {
    let codec = params.id();
    let context = codec::context::Context::from_parameters(params).with_context(|| {
        format!(
            "Failed to create codec context for `{}` codec",
            codec.name()
        )
    })?;

    context
        .decoder()
        .video()
        .with_context(|| format!("Failed to create decoder for `{}` codec", codec.name()))
}

pub fn extract_images_from_file<'a, P, I>(
    file: P,
    points: I,
    stream_idx: Option<usize>,
    sender: Sender<(String, DynamicImage)>,
    pb: ProgressBar,
) -> Result<()>
where
    P: AsRef<Path>,
    I: Iterator<Item = (Timestamp, &'a str)>,
{
    let ictx = libav::format::input(&file).context("Failed to open file")?;
    let stream = get_stream(ictx.streams(), media::Type::Video, stream_idx)?;
    let stream_idx = stream.index();
    trace!(
        "Using {} stream at index {}",
        stream.parameters().id().name(),
        stream_idx,
    );

    let decoder = create_decoder(stream.parameters())?;
    trace!("Created {} decoder", stream.parameters().id().name());

    let src_width = decoder.width();
    let src_height = decoder.height();

    let scaler = scaling::context::Context::get(
        decoder.format(),
        src_width,
        src_height,
        libav::format::pixel::Pixel::RGB24,
        src_width,
        src_height,
        scaling::flag::Flags::BILINEAR,
    )
    .context("Failed to create scaler context")?;

    trace!("Created sws scaler context");
    extract_images_from_stream(sender, ictx, decoder, scaler, points, stream_idx, pb)
}

pub fn write_images(receiver: Receiver<(String, DynamicImage)>) -> Result<()> {
    while let Ok((file, image)) = receiver.recv() {
        image
            .save(&file)
            .with_context(|| format!("{}: Failed to write image", file))?;
        trace!("{}: Wrote to file", file);
    }
    trace!("no more images to convert");
    Ok(())
}
