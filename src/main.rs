extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};
use crossbeam_channel::unbounded;
use log::{error, trace, warn};
use rayon::ThreadPoolBuilder;

mod args;
mod audio;
mod format;
mod image;
mod subtitle;
mod util;

use crate::image::*;
use args::Args;
use audio::generate_audio_commands;
use subtitle::{read_subtitles, Subtitle};
use format::Format;

fn main() -> Result<()> {
    let args = Args::parse_from_env()?;

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbosity)
        .init();
    trace!("initialized logger");

    libav::init().context("Failed to initialize libav")?;
    trace!("initialized libav");

    if args.sub_files.is_empty() {
        eprintln!("Usage: {} [OPTIONS] SUBTITLE_FILE...", args.executable);
        std::process::exit(0);
    }

    let subtitles = args
        .sub_files
        .iter()
        .map(|file| read_subtitles(file, args.sub_stream))
        .collect::<Result<Vec<Vec<Subtitle>>>>()?;
    trace!("read all subtitles from {} file(s)", subtitles.len());

    let media_files = if args.media_files.is_empty() {
        trace!("using subtitle files argument as media files");
        &args.sub_files
    } else {
        &args.media_files
    };

    let mut commands = if args.gen_audio {
        generate_audio_commands(
            media_files,
            &subtitles,
            args.audio_stream,
            &args.audio_format,
        )?
    } else {
        Default::default()
    };

    ThreadPoolBuilder::new().build().unwrap().scope_fifo(|s| {
        let (sender, receiver) = unbounded();
        for command in commands.iter_mut() {
            println!("{:?}", command);
            s.spawn_fifo(|_| match command.status() {
                Ok(exitcode) => {
                    if exitcode.success() {
                        trace!("a FFmpeg command exited successfully");
                    } else {
                        error!("a FFmpeg command exited with an error");
                    }
                }
                Err(err) => {
                    error!("failed to spawn command: {}", err);
                }
            });
        }

        if args.gen_image {
            std::iter::repeat(receiver).take(4).for_each(|receiver| {
                s.spawn_fifo(|_| match write_images(receiver) {
                    Ok(_) => {
                        trace!("converted images");
                    }
                    Err(err) => {
                        error!("failed  to convert images: {}", err);
                    }
                });
            });
            let file_count = media_files.len();

            std::iter::repeat(sender)
                .take(8)
                .zip(subtitles.iter())
                .zip(media_files.iter())
                .enumerate()
                .for_each(|(idx, ((sender, subs), media_file))| {
                    let mut format = Format::new(subs.len(), file_count, "").unwrap();
                    format.file_index = idx;
                    s.spawn_fifo(move |_| match extract_images(media_file, subs, format, sender) {
                        Ok(_) => {
                            trace!("{}: Decoded all images", media_file.to_string_lossy());
                        },
                        Err(err) => {
                            error!("{}: Failed to decode images: {}", media_file.to_string_lossy(), err);
                        },

                    });
                });
        }
    });
    Ok(())
}
