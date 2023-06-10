extern crate ffmpeg_next as ffmpeg;
extern crate pretty_env_logger;
mod subtitle;

use crate::subtitle::{Subtitle, SubtitleList};
use anyhow::{Context, Result};
use clap::Parser;
use glob::glob;
use log::{debug, error, info, trace, warn};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use ffmpeg::{
    codec, codec::packet::packet::Packet, decoder, media, util::mathematics::rescale::Rescale,
    util::rational::Rational,
};

#[derive(Debug)]
pub enum StosError {
    StreamSelectError(String),
    OpenDecoderError(ffmpeg::util::error::Error),
    NoStreamFound(media::Type),
    IncorrectStreamType {
        found: media::Type,
        expected: media::Type,
    },
    StreamOutOfBounds {
        index: usize,
        max: usize,
    },
    NoSubtitles,
    FFmpegError(std::process::ExitStatus),
    LabelMe,
}

impl Display for StosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            StosError::StreamSelectError(err) => write!(f, "Could not select stream: {}", err),
            StosError::NoStreamFound(medium) => {
                write!(f, "No {} stream present in file", medium_to_string(*medium))
            }
            StosError::IncorrectStreamType { found, expected } => write!(
                f,
                "Stream is of invalid type. Expected: {} found: {}",
                medium_to_string(*expected),
                medium_to_string(*found)
            ),
            StosError::StreamOutOfBounds { index, max } => {
                write!(f, "Stream index {} out of bounds (max {})", index, max)
            }
            StosError::NoSubtitles => write!(f, "The stream did not contain any subtitles"),
            StosError::FFmpegError(exit_status) => match exit_status.code() {
                Some(code) => write!(f, "FFmpeg did not exit successfully, exit code: {}", code),
                None => write!(f, "FFmpeg was killed by a signal"),
            },
            _ => todo!(),
        }
    }
}

impl std::error::Error for StosError {}

#[derive(Parser, Debug)]
struct Cli {
    file: String,

    sub_file: Option<String>,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,

    #[arg(short, long)]
    audio_index: Option<usize>,

    #[arg(short, long)]
    sub_index: Option<usize>,

    #[arg(short, long, default_value = "out_%f_%s.mka")]
    format: String,

    #[arg(short, long, help = "Combines overlapping subtitles into one")]
    coalesce: bool,

    #[arg(
        short,
        long = "print",
        help = "Print the command stos would execute and exit"
    )]
    print_command: bool,
}

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &Packet,
) -> Result<Option<codec::subtitle::Subtitle>, ffmpeg::util::error::Error> {
    let mut subtitle = codec::subtitle::Subtitle::default();
    match decoder.decode(packet, &mut subtitle) {
        Ok(true) => Ok(Some(subtitle)),
        Ok(false) => Ok(None),
        Err(err) => Err(err),
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Timestamp {
    ts: i64,
    time_base: Rational,
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let ts = self.ts.rescale(self.time_base, other.time_base);
        ts.cmp(&other.ts)
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Timestamp {
    pub fn new(ts: i64, time_base: Rational) -> Self {
        Self { ts, time_base }
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ts = self.ts.rescale(self.time_base, Rational::new(1, 1000000));
        write!(
            f,
            "{}:{:02}:{:02}.{:03}",
            ts / (1000 * 1000 * 60 * 60),
            (ts / (1000 * 1000 * 60)) % 60,
            (ts / (1000 * 1000)) % 60,
            (ts / 1000) % 1000
        )
    }
}

fn generate_command(
    subs: SubtitleList,
    audio_index: usize,
    format: &str,
    file_index: usize,
) -> Command {
    let mut command = Command::new("ffmpeg");

    for (idx, sub) in subs.subs().enumerate() {
        let start = Timestamp::new(sub.start, subs.time_base);
        let end = Timestamp::new(sub.end, subs.time_base);

        command.arg("-ss").arg(start.to_string());
        command.arg("-to").arg(end.to_string());
        command.arg("-map").arg(format!("0:{}", audio_index));
        command.arg(
            format
                .replace("%f", format!("{:02}", file_index).as_ref())
                .replace("%s", format!("{:03}", idx).as_ref()),
        );
        //command.arg(format!("out{:03}.mka", idx));
    }
    command
}

fn medium_to_string(medium: media::Type) -> &'static str {
    match medium {
        media::Type::Unknown => "unknown",
        media::Type::Video => "video",
        media::Type::Audio => "audio",
        media::Type::Data => "data",
        media::Type::Subtitle => "subtitle",
        media::Type::Attachment => "attachment",
    }
}

fn get_stream_index(file: &PathBuf, index: Option<usize>, medium: media::Type) -> Result<usize> {
    let ictx = ffmpeg::format::input(file)
        .with_context(|| format!("failed to open {}", file.to_string_lossy(),))?;

    let mut streams = ictx.streams();

    match index {
        None => Ok(streams
            .best(medium)
            .ok_or(StosError::NoStreamFound(medium))?
            .index()),
        Some(index) => {
            let stream = streams.nth(index).ok_or(StosError::StreamOutOfBounds {
                index,
                max: ictx.streams().count(),
            })?;

            if stream.parameters().medium() == medium {
                Ok(index)
            } else {
                Err(StosError::IncorrectStreamType {
                    found: stream.parameters().medium(),
                    expected: medium,
                }
                .into())
            }
        }
    }
}

//Gets all the subtitles from sub_file at stream index sub_index. Panics if stream index is a
//invalid stream index
fn get_subtitles(sub_file: &PathBuf, sub_index: usize) -> Result<SubtitleList> {
    let mut sub_ictx = ffmpeg::format::input(sub_file)
        .with_context(|| format!("failed to open {}", sub_file.to_string_lossy()))?;

    let (context, time_base) = {
        let stream = sub_ictx.streams().nth(sub_index).unwrap();
        (
            codec::context::Context::from_parameters(stream.parameters()).with_context(|| {
                format!(
                    "failed to create codec context for codec type {}",
                    stream.parameters().id().name()
                )
            })?,
            stream.time_base(),
        )
    };

    let mut decoder = context.decoder().subtitle().with_context(|| {
        format!(
            "failed to open decoder for codec type {}",
            sub_ictx
                .streams()
                .nth(sub_index)
                .unwrap()
                .parameters()
                .id()
                .name()
        )
    })?;

    trace!(
        "{}: Using codec {}",
        sub_file.to_string_lossy(),
        decoder.id().name()
    );

    let mut subs = SubtitleList::new(time_base);

    for (stream, packet) in sub_ictx.packets() {
        if stream.index() != sub_index {
            continue;
        }

        match decode_subtitle(&mut decoder, &packet).context("failed to decode subtitle packet")? {
            Some(subtitle) => match Subtitle::new(&subtitle, &packet, time_base) {
                Ok(subtitle) => {
                    trace!("Added a subtitle");
                    subs.add_sub(subtitle);
                }
                Err(err) => {
                    warn!("Failed to convert a subtitle: {}", err);
                }
            },
            None => {
                trace!("Did not get a subtitle this pass");
            }
        }
    }
    Ok(subs)
}

fn create_command(
    audio_file: &PathBuf,
    audio_index: Option<usize>,
    sub_file: &PathBuf,
    sub_index: Option<usize>,
    format: &str,
    coalesce: bool,
    file_index: usize,
) -> Result<std::process::Command> {
    let audio_index = get_stream_index(audio_file, audio_index, media::Type::Audio)?;
    debug!(
        "Selected audio stream at index {} from {}",
        audio_index,
        audio_file.to_string_lossy()
    );

    let sub_index =
        get_stream_index(sub_file, sub_index, media::Type::Subtitle).with_context(|| {
            format!(
                "{} failed to retrieve subtitle index",
                sub_file.to_string_lossy()
            )
        })?;
    debug!(
        "Selected subtitle stream at index {} from {}",
        sub_index,
        sub_file.to_string_lossy()
    );

    let mut subs = get_subtitles(sub_file, sub_index)?;
    debug!(
        "{}: Read {} subtitles",
        sub_file.to_string_lossy(),
        subs.len()
    );

    if subs.is_empty() {
        Err(StosError::NoSubtitles.into())
    } else {
        if coalesce {
            let before = subs.len();
            subs = subs.coalesce();
            debug!(
                "{}: Coalesced {} subtitles into {}",
                sub_file.to_string_lossy(),
                before,
                subs.len()
            );
        }

        let mut command = generate_command(subs, audio_index, format, file_index);

        command.arg("-i").arg(audio_file);
        command.arg("-loglevel").arg("warning");
        Ok(command)
    }
}

fn get_paths(pattern: &str) -> Result<impl Iterator<Item = PathBuf>> {
    Ok(glob(pattern)
        .with_context(|| format!("failed to match glob pattern \"{}\"", pattern))?
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry),
            Err(err) => {
                debug!("skipping a file: {}", err);
                None
            }
        }))
}

fn main() -> Result<()> {
    let args = Cli::parse();

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbose.log_level_filter())
        .init();

    ffmpeg::init().context("Failed to initalize FFmpeg")?;
    trace!("Initalized ffmpeg");

    let paths: Vec<PathBuf> = get_paths(&args.file)?.collect();
    debug!("glob \"{}\" matched {} file(s)", args.file, paths.len());

    let sub_paths = if let Some(pattern) = args.sub_file.as_ref() {
        let sub_paths: Vec<PathBuf> = get_paths(pattern)?.collect();
        debug!(
            "glob \"{}\" matched {} subtitle file(s)",
            args.file,
            sub_paths.len()
        );
        sub_paths
    } else {
        debug!(
            "no pattern specified for subtitle files, will use \"{}\"",
            args.file
        );
        paths.clone()
    };

    if paths.is_empty() {
        error!("no paths to convert");
        std::process::exit(1);
    }

    let paths: Vec<(usize, PathBuf, PathBuf)> = paths
        .into_iter()
        .zip(sub_paths)
        .enumerate()
        .map(|(idx, (path, sub_path))| (idx, path, sub_path))
        .collect();

    thread::scope(|s| -> Result<()> {
        let mut threads = Vec::new();
        for (idx, path, sub_path) in paths.iter() {
            threads.push(s.spawn(|| -> Result<()> {
                let mut command = create_command(
                    path,
                    args.audio_index,
                    sub_path,
                    args.sub_index,
                    args.format.as_ref(),
                    args.coalesce,
                    *idx,
                )?;
                debug!(
                    "generated audio extract command for {}",
                    path.to_string_lossy()
                );

                if args.print_command {
                    println!("{:?}", command);
                    Ok(())
                } else {
                    match command.status() {
                        Ok(status) => {
                            if status.success() {
                                info!(
                                    "successfully extracted audio from {}",
                                    path.to_string_lossy()
                                );
                            } else {
                                error!("{}: FFmpeg exited with an error", path.to_string_lossy());
                            }
                        }
                        Err(err) => {
                            error!("failed to start ffmpeg command: {:?}", err);
                        }
                    }
                    Ok(())
                }
            }));
        }

        for thread in threads {
            match thread.join() {
                Ok(result) => {
                    result?;
                }
                Err(_) => return Err(StosError::LabelMe.into()),
            }
        }
        Ok(())
    })?;
    Ok(())
}
