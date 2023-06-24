use crate::format::Format;
use crate::subtitle::Subtitle;
use crate::util::get_stream;
use crate::util::Timestamp;
use anyhow::{Context, Error, Result};
use crossbeam_channel::{Receiver, Sender};
use image::RgbImage;
use libav::{media, software::scaling, util::frame};
use log::{debug, trace, warn};
use std::path::PathBuf;

pub fn write_images(receiver: Receiver<(String, RgbImage)>) -> Result<()> {
    while let Ok((file, image)) = receiver.recv() {
        image
            .save(&file)
            .with_context(|| format!("{}: Failed to write image", file))?;
        trace!("{}: Wrote to file", file);
    }
    trace!("no more images to convert");
    Ok(())
}

pub fn extract_images(
    file: &PathBuf,
    subs: &Vec<Subtitle>,
    mut format: Format<'_>,
    sender: Sender<(String, RgbImage)>,
) -> Result<()> {
    let file_str = file.to_string_lossy();

    let mut ictx = libav::format::input(file).context("Failed to open file")?;
    debug!("Opened {}", file_str);

    let stream = get_stream(ictx.streams(), media::Type::Video, None)?;
    let stream_index = stream.index();
    let codec = stream.parameters().id();
    let timebase = stream.time_base();

    debug!(
        "{}: Using video stream at index {}. Codec: {}",
        file_str,
        stream_index,
        codec.name()
    );

    let context = libav::codec::context::Context::from_parameters(stream.parameters())
        .with_context(|| format!("Failed to create codec context for {} codec", codec.name()))?;
    trace!(
        "{}: Created codec context for {} codec type",
        file_str,
        codec.name()
    );

    let mut decoder = context.decoder().video().with_context(|| {
        format!(
            "{}: Failed to open decoder for {} codec type",
            file_str,
            codec.name()
        )
    })?;
    trace!("{}: Opened {} decoder", file_str, codec.name());

    let mut scaler = scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        libav::format::pixel::Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        scaling::flag::Flags::BILINEAR,
    )
    .context("Failed to create sws scaler context")?;
    trace!("{}: Created sws scaler context", file_str);

    let mut receive_and_process_frame = |decoder: &mut libav::decoder::Video| -> Result<bool> {
        let mut decoded = frame::video::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            if format.sub_index >= subs.len() {
                return Ok(false);
            }

            if Timestamp::from_timebase(decoded.pts().unwrap_or(0), timebase).unwrap()
                < subs[format.sub_index].start
            {
                continue;
            }

            let mut rgb_frame = frame::video::Video::empty();
            scaler
                .run(&decoded, &mut rgb_frame)
                .context("Failed to scale frame")?;

            let image = RgbImage::from_raw(
                decoder.width(),
                decoder.height(),
                rgb_frame.data(0).to_vec(),
            )
            .ok_or(Error::msg("Failed to convert frame to image"))?;

            sender
                .send((format.to_string(), image))
                .context("Failed to send image")?;

            format.sub_index += 1;
        }
        Ok(true)
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
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
        .context("Failed to send eof to decoder")?;
    receive_and_process_frame(&mut decoder)?;

    if format.sub_index < subs.len() {
        warn!(
            "{}: Was only able to extract {} images",
            file_str, format.sub_index
        );
    }
    Ok(())
}
