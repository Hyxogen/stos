extern crate ffmpeg_next as libav;
use anyhow::{bail, Context, Result};
use crossbeam_channel::{unbounded, Sender};
use genanki_rs::{Deck, Package};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::{error, trace, Log};
use rayon::prelude::*;
use std::path::PathBuf;

mod anki;
mod args;
mod ass;
mod audio;
mod image;
mod subtitle;
mod time;
mod util;

use crate::image::{extract_images_from_file, write_images};
use anki::create_notes;
use args::Args;
use audio::generate_audio_commands;
use subtitle::{read_subtitles_from_file, Dialogue, Subtitle};
use time::{Timespan, Timestamp};

pub struct SubtitleBundle {
    sub: Subtitle,
    sub_image: Option<String>,
    audio: Option<String>,
    image: Option<String>,
}

impl From<Subtitle> for SubtitleBundle {
    fn from(sub: Subtitle) -> Self {
        Self {
            sub,
            sub_image: None,
            audio: None,
            image: None,
        }
    }
}

impl SubtitleBundle {
    pub fn sub(&self) -> &Subtitle {
        &self.sub
    }

    pub fn sub_image(&self) -> Option<&str> {
        self.sub_image.as_deref()
    }

    pub fn set_sub_image(&mut self, sub_image: &str) -> &mut Self {
        self.sub_image = Some(sub_image.to_string());
        self
    }

    pub fn audio(&self) -> Option<&str> {
        self.audio.as_deref()
    }

    pub fn set_audio(&mut self, audio: &str) -> &mut Self {
        self.audio = Some(audio.to_string());
        self
    }

    pub fn image(&self) -> Option<&str> {
        self.image.as_deref()
    }

    pub fn set_image(&mut self, image: &str) -> &mut Self {
        self.image = Some(image.to_string());
        self
    }
}

enum Job<'a, 'b> {
    Command {
        pb: ProgressBar,
        command: std::process::Command,
    },
    WriteImage {
        path: &'a std::path::Path,
        image: &'b image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    },
    ExtractImages {
        pb: ProgressBar,
        path: &'a PathBuf,
        points: Vec<(Timestamp, &'b str)>,
        stream_idx: Option<usize>,
        sender: Sender<(String, image::DynamicImage)>,
    },
}

impl<'a, 'b> Job<'a, 'b> {
    pub fn execute(self) -> Result<()> {
        match self {
            Job::Command { pb, command } => {
                Self::execute_command(command)?;
                pb.inc(1);
                Ok(())
            }
            Job::WriteImage { path, image } => {
                Ok(image.save(path).context("Failed to save image")?)
            }
            Job::ExtractImages {
                pb,
                path,
                points,
                stream_idx,
                sender,
            } => extract_images_from_file(path, points.into_iter(), stream_idx, sender, pb)
                .with_context(|| {
                    format!(
                        "Failed to extract images from \"{}\"",
                        path.to_string_lossy()
                    )
                }),
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

fn run<L: Log + 'static>(args: &Args, logger: L) -> Result<()> {
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

    if args.sub_files().is_empty() {
        bail!("no subtitle files specified");
    }
    if media_files.len() != args.sub_files().len() {
        bail!("the amount of media files must be the same as the amount of subtitle files");
    }

    let max_file_width = (media_files.len().ilog10() + 1) as usize;

    let subtitles = args
        .sub_files()
        .iter()
        .map(|file| {
            read_subtitles_from_file(&file, args.sub_stream()).with_context(|| {
                format!(
                    "Failed to read subtitles from \"{}\"",
                    file.to_string_lossy()
                )
            })
        })
        .map(|result| result.map(|subs| subs.collect()))
        .collect::<Result<Vec<Vec<Subtitle>>>>()?;

    let mut subtitles: Vec<Vec<SubtitleBundle>> = subtitles
        .into_iter()
        .map(|subs| {
            subs.into_iter()
                .filter(|sub| sub.timespan().start() >= args.start())
                .filter(|sub| sub.timespan().start() <= args.end())
                .filter(|sub| {
                    !sub.text()
                        .map(|text| args.blacklist().iter().any(|re| re.is_match(text)))
                        .unwrap_or(false)
                })
                .filter(|sub| {
                    if args.whitelist().is_empty() {
                        true
                    } else {
                        sub.text()
                            .map(|text| args.whitelist().iter().any(|re| re.is_match(text)))
                            .unwrap_or(false)
                    }
                })
                .map(Into::into)
                .collect()
        })
        .collect();

    let mut jobs: Vec<Job> = Vec::new();

    for (file_idx, subs) in subtitles.iter_mut().enumerate() {
        let max_index = subs.len();
        let max_width: usize = (max_index.ilog10() + 1) as usize;

        for (sub_idx, sub) in subs.iter_mut().enumerate() {
            if let Dialogue::Bitmap(_) = sub.sub().dialogue() {
                sub.set_sub_image(&format!(
                    "sub_{:0max_file_width$}_{:0max_width$}.png",
                    file_idx, sub_idx
                ));
            }

            if args.gen_audio() {
                sub.set_audio(&format!(
                    "audio_{:0max_file_width$}_{:0max_width$}.mka",
                    file_idx, sub_idx
                ));
            }
            if args.gen_images() {
                sub.set_image(&format!(
                    "image_{:0max_file_width$}_{:0max_width$}.jpg",
                    file_idx, sub_idx
                ));
            }
        }
    }

    let (sender, receiver) = unbounded();

    let m = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{msg:9!} [{elapsed_precise}] {bar:50.cyan/blue} {percent:>4}% [eta {eta:<}]",
    )
    .unwrap()
    .progress_chars("##-");
    LogWrapper::new(m.clone(), logger).try_init().unwrap();
    let audio_pb = m.add(ProgressBar::new(0));
    audio_pb.set_message("audio");
    audio_pb.set_style(style.clone());

    for (sender, (file, subs)) in
        std::iter::repeat(sender).zip(media_files.iter().zip(subtitles.iter()))
    {
        let commands = generate_audio_commands(
            file,
            subs.iter().filter_map(|bundle| {
                bundle.audio().map(|out_file| {
                    let span = bundle.sub().timespan();
                    (
                        Timespan::new(
                            span.start().saturating_sub(args.pad_begin()),
                            span.end().saturating_add(args.pad_end()),
                        ),
                        out_file,
                    )
                })
            }),
            args.audio_stream(),
        )?;
        audio_pb.inc_length(commands.len().try_into().unwrap());

        for command in commands {
            jobs.push(Job::Command {
                pb: audio_pb.clone(),
                command,
            });
        }
        //jobs.extend(tmp.into_iter().map(Into::into));
        let image_pb = m.add(ProgressBar::new(subs.len().try_into().unwrap()));
        image_pb.set_style(style.clone());
        image_pb.set_message(file.file_stem().unwrap().to_string_lossy().to_string());

        jobs.push(Job::ExtractImages {
            pb: image_pb.clone(),
            path: file,
            points: subs
                .iter()
                .filter_map(|bundle| {
                    bundle
                        .image()
                        .map(|out_file| (bundle.sub().timespan().start(), out_file))
                })
                .collect(),
            stream_idx: args.video_stream(),
            sender,
        });

        for sub in subs {
            if let (Dialogue::Bitmap(image), Some(path)) = (sub.sub().dialogue(), sub.sub_image()) {
                jobs.push(Job::WriteImage {
                    path: path.as_ref(),
                    image,
                });
            }
        }
    }

    trace!("generated {} jobs", jobs.len());

    std::thread::scope(|s| -> Result<()> {
        std::iter::repeat(receiver).take(5).for_each(|receiver| {
            s.spawn(|| match write_images(receiver) {
                Ok(_) => {
                    trace!("converted images");
                }
                Err(err) => {
                    error!("failed to convert images: {:?}", err);
                }
            });
        });

        jobs.into_par_iter()
            .map(Job::execute)
            .collect::<Result<_>>()
    })?;

    audio_pb.finish_with_message("done");

    trace!("executed all jobs");

    let notes = create_notes(subtitles.iter().flat_map(|subs| subs.iter()))?;
    trace!("creates {} notes", notes.len());

    let mut deck = Deck::new(args.deck_id(), args.deck_name(), args.deck_desc());
    trace!("created anki deck");

    for note in notes {
        deck.add_note(note);
    }

    let assets = subtitles
        .iter()
        .flat_map(|subs| subs.iter())
        .flat_map(|sub| {
            let mut assets = Vec::new();
            if let Some(sub_image) = sub.sub_image() {
                assets.push(sub_image);
            }
            if let Some(image) = sub.image() {
                assets.push(image);
            }
            if let Some(audio) = sub.audio() {
                assets.push(audio);
            }
            assets.into_iter()
        });

    let mut package =
        Package::new(vec![deck], assets.collect()).context("Failed to create anki package")?;
    trace!("created package");

    package
        .write_to_file(args.package())
        .context("Failed to write package to file")?;

    //read subtitles
    //filter/transform subtitles
    //generate media
    //generate deck
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse_from_env()?;

    let logger = pretty_env_logger::formatted_builder()
        .filter_level(args.verbosity())
        .build();
    trace!("initialized logger");
    //execute

    libav::init().context("Failed to initialize libav")?;

    run(&args, logger)?;
    /*
    if let Err(error) = run() {
        //print pretty error
    }*/
    Ok(())
}
