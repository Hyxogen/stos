extern crate ffmpeg_next as libav;
use anyhow::{bail, Context, Result};
use log::trace;
use rayon::prelude::*;
use std::path::PathBuf;

mod args;
mod ass;
mod audio;
mod subtitle;
mod time;
mod util;

use args::Args;
use audio::generate_audio_commands;
use subtitle::{read_subtitles_from_file, Dialogue, Subtitle};

enum Job<'a, 'b> {
    Command(std::process::Command),
    WriteImage {
        path: &'a PathBuf,
        image: &'b image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    },
}

impl<'a, 'b> From<std::process::Command> for Job<'a, 'b> {
    fn from(c: std::process::Command) -> Self {
        Self::Command(c)
    }
}

impl<'a, 'b> Job<'a, 'b> {
    pub fn execute(self) -> Result<()> {
        match self {
            Job::Command(command) => Self::execute_command(command),
            Job::WriteImage { .. } => todo!(),
        }
    }

    fn execute_command(mut command: std::process::Command) -> Result<()> {
        match command
            .status()
            .context("Failed to execute command")?
            .success()
        {
            true => Ok(()),
            false => bail!("FFmpeg exited with an error"),
        }
    }
}

fn run(args: &Args) -> Result<()> {
    trace!(
        "extracting subtitles form {} file(s)",
        args.sub_files().len()
    );

    let media_files = if !args.media_files().is_empty() {
        args.media_files()
    } else {
        trace!("will use subtitle files argument as media files");
        args.sub_files()
    };

    if args.sub_files().len() == 0 {
        bail!("no subtitle files specified");
    }
    if media_files.len() != args.sub_files().len() {
        bail!("the amount of media files must be the same as the amount of subtitle files");
    }

    let max_file_width = (media_files.len().ilog10() + 1) as usize;

    let subtitles = args
        .sub_files()
        .iter()
        .map(|file| read_subtitles_from_file(&file, args.sub_stream()))
        .map(|result| result.map(|subs| subs.collect()))
        .collect::<Result<Vec<Vec<Subtitle>>>>()?;

    let mut jobs: Vec<Job> = Vec::new();

    if args.gen_audio() {
        trace!("generating audio file names");
        let audio_names: Vec<Vec<String>> = subtitles
            .iter()
            .enumerate()
            .map(|(file_idx, subs)| {
                if !subs.is_empty() {
                    let max_index = subs.len();
                    let max_width: usize = (max_index.ilog10() + 1) as usize;

                    (0..max_index)
                        .map(|sub_idx| {
                            format!(
                                "audio_{:0max_file_width$}_{:0max_width$}.mka",
                                file_idx, sub_idx
                            )
                        })
                        .collect()
                } else {
                    Default::default()
                }
            })
            .collect();

        trace!("generating FFmpeg commands to extract audio");
        for ((file, subs), names) in media_files
            .iter()
            .zip(subtitles.iter())
            .zip(audio_names.iter())
        {
            jobs.extend(
                generate_audio_commands(
                    file,
                    subs.iter().map(Subtitle::timespan).zip(names.iter()),
                    args.audio_stream(),
                )?
                .into_iter()
                .map(Into::into),
            );
        }
    } else {
        trace!("not extracting audio");
    }

    trace!("generated {} jobs", jobs.len());
    jobs.into_par_iter()
        .map(Job::execute)
        .collect::<Result<_>>()?;
    trace!("executed all jobs");

    //read subtitles
    //filter/transform subtitles
    //generate media
    //generate deck
    Ok(())
}

fn main() -> Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let args = Args::parse_from_env()?;
    //execute

    libav::init().context("Failed to initialize libav")?;

    run(&args)?;
    /*
    if let Err(error) = run() {
        //print pretty error
    }*/
    Ok(())
}
