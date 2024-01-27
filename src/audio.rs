use crate::time::Timespan;
use crate::util::{get_stream, StreamSelector};
use anyhow::{Context, Result};
use itertools::Itertools;
use libav::media;
use log::trace;
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::Command;

fn generate_audio_command_from_stream<'a, P, I>(path: P, points: I, stream_idx: usize) -> Command
where
    P: AsRef<Path>,
    I: Iterator<Item = (Timespan, &'a str)>,
{
    let mut command = Command::new("ffmpeg");

    let stream_map = format!("0:{}", stream_idx);

    for (span, name) in points {
        command.arg("-ss").arg(span.start().to_string());
        command.arg("-to").arg(span.end().to_string());
        command.arg("-map").arg(&stream_map);
        command.arg(name);
    }

    command.arg("-loglevel").arg("warning");
    command.arg("-i").arg(path.as_ref());

    command
}

fn generate_audio_commands_from_stream_chunked<'a, P, I>(
    path: P,
    points: I,
    stream_idx: usize,
    chunk_size: NonZeroUsize,
) -> Vec<Command>
where
    P: AsRef<Path>,
    I: Iterator<Item = (Timespan, &'a str)>,
{
    points
        .chunks(chunk_size.into())
        .into_iter()
        .map(|chunk| generate_audio_command_from_stream(&path, chunk, stream_idx))
        .collect()
}

pub fn generate_audio_commands<'a, P, I>(
    path: P,
    points: I,
    selector: StreamSelector<'_>,
) -> Result<Vec<Command>>
where
    P: AsRef<Path>,
    I: Iterator<Item = (Timespan, &'a str)>,
{
    let ictx = libav::format::input(&path).context("Failed to open file")?;
    let stream = get_stream(ictx.streams(), media::Type::Audio, selector)?;
    trace!(
        "Using {} stream at index {}",
        stream.parameters().id().name(),
        stream.index()
    );

    Ok(generate_audio_commands_from_stream_chunked(
        path,
        points,
        stream.index(),
        32usize.try_into().unwrap(),
    ))
}
