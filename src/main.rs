extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};
use log::trace;

mod args;
mod format;
mod subtitle;
mod util;
mod audio;

use args::Args;
use subtitle::{read_subtitles, Subtitle};
use audio::generate_audio_commands;

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

    let commands = if args.gen_audio {
        let audio_files = if !args.media_files.is_empty() {
            &args.media_files
        } else {
            trace!("using subtitle files argument as media files");
            &args.sub_files
        };
        generate_audio_commands(
            audio_files,
            &subtitles,
            args.audio_stream,
            &args.audio_format,
        )?
    } else {
        Default::default()
    };

    println!("subtitle files");
    for file in args.sub_files {
        println!("{:?}", file);
    }
    println!("media files");
    for file in args.media_files {
        println!("{:?}", file);
    }
    Ok(())
}
