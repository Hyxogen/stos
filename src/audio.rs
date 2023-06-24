use crate::format::Format;
use crate::subtitle::Subtitle;
use crate::util::get_stream;
use anyhow::{Context, Result};
use libav::media;
use log::{debug, trace, warn};
use std::path::PathBuf;
use std::process::Command;

fn generate_audio_command(
    file: &PathBuf,
    subs: &Vec<Subtitle>,
    stream_index: Option<usize>,
    mut format: Format<'_>,
) -> Result<Command> {
    let file_str = file.to_string_lossy();

    let ictx = libav::format::input(file).context("Failed to open file")?;
    trace!("Opened {}", file_str);

    let stream_index = get_stream(ictx.streams(), media::Type::Audio, stream_index)
        .with_context(|| format!("{}: Failed to retrieve audio stream", file_str))?
        .index();
    debug!("{}: Using audio stream at index {}", file_str, stream_index);

    let mut command = Command::new("ffmpeg");
    for (idx, subtitle) in subs.iter().enumerate() {
        format.sub_index = idx;

        command.arg("-ss").arg(subtitle.start.to_string());
        command.arg("-to").arg(subtitle.end.to_string());
        command.arg("-map").arg(format!("0:{}", stream_index));
        command.arg(format.to_string());
    }

    command.arg("-loglevel").arg("warning");
    command.arg("-i").arg(file);
    trace!(
        "{}: Generated a command to extract audio at {} positions",
        file_str,
        subs.len()
    );
    Ok(command)
}

pub fn generate_audio_commands(
    audio_files: &Vec<PathBuf>,
    subtitles: &Vec<Vec<Subtitle>>,
    audio_stream: Option<usize>,
    format: &str,
) -> Result<Vec<Command>> {
    if audio_files.len() != subtitles.len() {
        warn!("amount of subtitle files ({}) does not match the amount of media files ({}), will only extract audio from the first {} media files", subtitles.len(), audio_files.len(), subtitles.len().min(audio_files.len()));
    }

    let mut commands = Vec::new();

    for (file_index, (audio_file, subtitles)) in audio_files.iter().zip(subtitles).enumerate() {
        let mut format = Format::new(subtitles.len(), audio_files.len(), format)?;
        format.file_index = file_index;
        commands.push(
            generate_audio_command(audio_file, subtitles, audio_stream, format).with_context(
                || {
                    format!(
                        "Failed to generate command to extract audio from {}",
                        audio_file.to_string_lossy()
                    )
                },
            )?,
        );
    }
    trace!("Generated {} commands to extract audio", commands.len());
    Ok(commands)
}
