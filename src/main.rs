extern crate ffmpeg_next as libav;
use anyhow::{Context, Error, Result};
use crossbeam_channel::{unbounded, Sender};
use genanki_rs::{Deck, Package};
use log::{debug, error, info, trace};
use rayon::prelude::*;
use std::path::PathBuf;
use std::process;

mod anki;
mod args;
mod ass;
mod audio;
mod format;
mod image;
mod subtitle;
mod util;

use crate::image::*;
use anki::*;
use args::Args;
use audio::generate_audio_commands;
use format::Format;
use subtitle::{merge_overlapping, read_subtitles, Rect, Subtitle};

enum Job<'a> {
    Command(std::process::Command),
    WriteImage {
        path: PathBuf,
        image: &'a image::ImageBuffer<Rgba<u8>, Vec<u8>>,
    },
    DecodeVideo {
        file: PathBuf,
        format: Format<'a>,
        subs: Vec<Subtitle>,
        sender: Sender<(String, image::DynamicImage)>,
    },
}

impl<'a> From<process::Command> for Job<'a> {
    fn from(c: process::Command) -> Self {
        Self::Command(c)
    }
}

impl<'a> Job<'a> {
    pub fn execute(self) -> Result<()> {
        match self {
            Job::Command(command) => Self::execute_command(command),
            Job::WriteImage { path, image } => {
                image
                    .save(&path)
                    .with_context(|| format!("{}: Failed to save image", path.to_string_lossy()))?;
                Ok(())
            }
            Job::DecodeVideo {
                file,
                format,
                subs,
                sender,
            } => extract_images(&file, &subs, format, sender),
        }
    }

    fn execute_command(mut command: process::Command) -> Result<()> {
        match command
            .status()
            .context("Failed to execute command")?
            .success()
        {
            true => Ok(()),
            false => Err(Error::msg("FFmpeg exited with an error")),
        }
    }
}

fn filter_subs(subtitles: Vec<Vec<Subtitle>>, args: &Args) -> Vec<Vec<Subtitle>> {
    subtitles
        .into_iter()
        .map(|subs| {
            if !args.whitelist.is_empty() {
                subs.into_iter()
                    .map(|sub| {
                        let rects = sub
                            .rects
                            .into_iter()
                            .filter(|rect| match rect {
                                Rect::Text(text) => {
                                    args.whitelist.iter().any(|re| re.is_match(text))
                                }
                                Rect::Ass(ass) => args
                                    .whitelist
                                    .iter()
                                    .any(|re| re.is_match(&ass.text.dialogue)),
                                _ => true,
                            })
                            .collect();

                        Subtitle {
                            rects,
                            start: sub.start,
                            end: sub.end,
                        }
                    })
                    .filter(|sub| !sub.rects.is_empty())
                    .collect()
            } else {
                subs
            }
        })
        .map(|subs| {
            if !args.blacklist.is_empty() {
                subs.into_iter()
                    .map(|sub| {
                        let rects = sub
                            .rects
                            .into_iter()
                            .filter(|rect| match rect {
                                Rect::Text(text) => {
                                    args.blacklist.iter().any(|re| !re.is_match(text))
                                }
                                Rect::Ass(ass) => args
                                    .blacklist
                                    .iter()
                                    .any(|re| !re.is_match(&ass.text.dialogue)),
                                _ => true,
                            })
                            .collect();

                        Subtitle {
                            rects,
                            start: sub.start,
                            end: sub.end,
                        }
                    })
                    .filter(|sub| !sub.rects.is_empty())
                    .collect()
            } else {
                subs
            }
        })
        .map(|subs| {
            if args.coalesce {
                merge_overlapping(subs)
            } else {
                subs
            }
        })
        .collect()
}

fn main() -> Result<()> {
    let args = Args::parse_from_env()?;

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbosity)
        .init();
    trace!("initialized logger");

    libav::init().context("Failed to initialize libav")?;
    trace!("initialized libav");

    let subtitles = args
        .sub_files
        .iter()
        .map(|file| read_subtitles(file, args.sub_stream))
        .collect::<Result<Vec<Vec<Subtitle>>>>()?;
    trace!("read all subtitles from {} file(s)", subtitles.len());

    if subtitles.iter().all(|list| list.is_empty()) {
        return Err(Error::msg("The file(s) did not contain any subtitles"));
    }

    let subtitles = filter_subs(subtitles, &args);

    if subtitles.iter().all(|list| list.is_empty()) {
        return Err(Error::msg(
            "None of the subtitles matched the whitelist and/or all subtitles were blacklisted",
        ));
    }

    let media_files = if args.media_files.is_empty() {
        trace!("using subtitle files argument as media files");
        &args.sub_files
    } else {
        &args.media_files
    };

    let mut jobs: Vec<Job> = if args.gen_audio {
        trace!("generating FFmpeg commands to extract audio");
        generate_audio_commands(
            media_files,
            &subtitles,
            args.audio_stream,
            &args.audio_format,
        )?
        .into_iter()
        .map(Into::into)
        .collect()
    } else {
        trace!("not extracting audio");
        Default::default()
    };

    for (file_idx, list) in subtitles.iter().enumerate() {
        let mut format = Format::new(list.len(), subtitles.len(), &args.sub_format).unwrap();
        format.file_index = file_idx;
        for (sub_idx, sub) in list.iter().enumerate() {
            format.sub_index = sub_idx;
            for (rect_idx, rect) in sub.rects.iter().enumerate() {
                format.set_rect_count(sub.rects.len()).unwrap();
                format.rect_index = rect_idx;
                if let Rect::Bitmap(image) = rect {
                    jobs.push(Job::WriteImage {
                        path: format.to_string().into(),
                        image,
                    });
                }
            }
        }
    }

    let (sender, receiver) = unbounded();
    let media_file_count = media_files.len();

    if args.gen_image {
        trace!("generating jobs to extract images");
        jobs.extend(
            std::iter::repeat(sender)
                .zip(subtitles.clone())
                .zip(media_files.clone())
                .enumerate()
                .map(|(idx, ((sender, subs), file))| {
                    let mut format =
                        Format::new(subs.len(), media_file_count, &args.image_format).unwrap();
                    format.file_index = idx;
                    Job::DecodeVideo {
                        file,
                        subs,
                        format,
                        sender,
                    }
                }),
        );
    } else {
        drop(sender);
        trace!("not extracting images");
    }

    trace!("will execute {} jobs in parallel", jobs.len());

    if !args.no_media {
        std::thread::scope(|s| -> Result<()> {
            std::iter::repeat(receiver).take(12).for_each(|receiver| {
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
                .collect::<Result<Vec<_>>>()?;
            Ok(())
        })?;
    } else {
        debug!("did not output any media files because \"--no-media\" was specified");
    }
    trace!("executed all jobs");

    let (notes, media) = create_all_notes(
        &subtitles,
        &args.sub_format,
        args.gen_image.then_some(&args.image_format),
        args.gen_audio.then_some(&args.audio_format),
    )?;
    trace!("created {} notes", notes.len());

    let mut deck = Deck::new(args.deck_id, &args.deck_name, &args.deck_desc);
    trace!("created anki deck \"{}\"", args.deck_name);

    for note in notes {
        deck.add_note(note);
    }

    let mut package = Package::new(vec![deck], media.iter().map(|x| x.as_str()).collect())
        .context("failed to create anki package")?;
    trace!("created package");

    if !args.no_deck {
        package.write_to_file(&args.package).with_context(|| {
            format!(
                "{}: Failed to save package to file",
                args.package.to_string_lossy()
            )
        })?;
        info!("wrote package to {}", &args.package.to_string_lossy());
    } else {
        debug!("did not output a package files because \"--no-deck\" was specified");
    }
    Ok(())
}
