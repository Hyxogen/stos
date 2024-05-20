extern crate ffmpeg_next as libav;
use anyhow::{bail, Context, Result};
use crossbeam_channel::{unbounded, Sender};
use genanki_rs::{Deck, Package};
use human_panic::setup_panic;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::{error, trace, warn};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use serde::Serialize;
use std::collections::HashMap;
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
use time::{Duration, Timespan, Timestamp};
use util::StreamSelector;

#[derive(Serialize)]
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

enum Job<'a, 'b, 'c> {
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
        selector: StreamSelector<'c>,
        sender: Sender<(String, image::DynamicImage)>,
    },
}

impl<'a, 'b, 'c> Job<'a, 'b, 'c> {
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
                selector,
                sender,
            } => extract_images_from_file(path, points.into_iter(), selector, sender, pb)
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

fn merge_overlapping<I>(subs: I, max_dist: Duration) -> Vec<Subtitle>
where
    I: Iterator<Item = Subtitle>,
{
    let mut result: Vec<Subtitle> = Vec::new();
    let mut diags: HashMap<Dialogue, usize> = HashMap::new();
    let mut count = 0;

    for sub in subs {
        count += 1usize;
        if let Some(idx) = diags.get(sub.dialogue()) {
            let prev_sub = &mut result[*idx];
            if prev_sub.timespan().end() + max_dist >= sub.timespan().start() {
                prev_sub.set_timespan(Timespan::new(
                    prev_sub.timespan().start(),
                    sub.timespan().end(),
                ));
                continue;
            }
        }
        diags.insert(sub.dialogue().clone(), result.len());
        result.push(sub);
    }

    trace!("merged {} subs into {}", count, result.len());

    result
}

fn read_subtitles(args: &Args) -> Result<Vec<Vec<Subtitle>>> {
    args.sub_files()
        .iter()
        .map(|file| {
            read_subtitles_from_file(&file, args.sub_stream_selector()).with_context(|| {
                format!(
                    "Failed to read subtitles from \"{}\"",
                    file.to_string_lossy()
                )
            })
        })
        .map(|result| result.map(|subs| subs.collect()))
        .collect()
}

fn process_subtitles(args: &Args, subs: Vec<Subtitle>) -> Vec<SubtitleBundle> {
    let subs = if args.merge_subs() {
        trace!("merging subtitles");
        merge_overlapping(subs.into_iter(), args.merge_diff())
    } else {
        trace!("not merging subtitles");
        subs
    };

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
        .filter(|sub| {
            if let Dialogue::Ass(ass) = sub.dialogue() {
                !args.ignore_styled() || !ass.text.is_styled()
            } else {
                true
            }
        })
        .map(Into::into)
        .collect()
}

fn run(args: &Args, multi: MultiProgress) -> Result<()> {
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

    trace!("got {} media file(s)", media_files.len());
    if media_files.len() != args.sub_files().len() {
        bail!("the amount of media files must be the same as the amount of subtitle files");
    }

    let max_file_width = (media_files.len().ilog10() + 1) as usize;

    let subtitles = read_subtitles(args)?;
    let mut subtitles: Vec<Vec<SubtitleBundle>> = subtitles
        .into_iter()
        .map(|subs| process_subtitles(args, subs))
        .collect();

    if subtitles.iter().all(|arr| arr.is_empty()) {
        warn!("All subtitles were ignored due to filter specified");
    }

    let audio_files: Vec<Vec<(Timespan, String)>> = subtitles
        .iter_mut()
        .enumerate()
        .map(|(file_idx, subs)| {
            let mut audio_files: Vec<(Timespan, String)> = Vec::new();

            if subs.is_empty() || !args.gen_audio() {
                return audio_files;
            }

            let max_index = subs.len();
            let max_width: usize = (max_index.ilog10() + 1) as usize;
            let mut sub_idx = 0usize;
            let count_before = subs.len();

            for sub in subs {
                let sub_span = sub.sub().timespan();
                let sub_span = Timespan::new(
                    sub_span
                        .start()
                        .saturating_sub(args.pad_begin())
                        .saturating_add(args.shift_audio()),
                    sub_span
                        .end()
                        .saturating_add(args.pad_end())
                        .saturating_add(args.shift_audio()),
                );

                if args.join_audio() {
                    if let Some((span, name)) = audio_files.last_mut() {
                        if span.end() >= sub_span.start() {
                            *span = Timespan::new(span.start(), sub_span.end());
                            sub.set_audio(name);
                            continue;
                        }
                    }
                }

                let file_name = format!(
                    "audio_{:0max_file_width$}_{:0max_width$}.mka",
                    file_idx, sub_idx
                );
                sub.set_audio(&file_name);
                audio_files.push((sub_span, file_name));
                sub_idx += 1;
            }
            trace!(
                "joined {} audio files into {}",
                count_before,
                audio_files.len()
            );
            audio_files
        })
        .collect();

    let mut jobs: Vec<Job> = Vec::new();

    for (file_idx, subs) in subtitles.iter_mut().enumerate() {
        if subs.is_empty() {
            continue;
        }

        let max_index = subs.len();
        let max_width: usize = (max_index.ilog10() + 1) as usize;

        for (sub_idx, sub) in subs.iter_mut().enumerate() {
            if let Dialogue::Bitmap(_) = sub.sub().dialogue() {
                sub.set_sub_image(&format!(
                    "sub_{:0max_file_width$}_{:0max_width$}.jpg",
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

    let style = ProgressStyle::with_template(
        "{msg:9!} [{elapsed_precise}] {bar:50.cyan/blue} {percent:>4}% [eta {eta:<}]",
    )
    .unwrap()
    .progress_chars("##-");
    let audio_pb = multi.add(ProgressBar::new(0));
    audio_pb.set_message("audio");
    audio_pb.set_style(style.clone());

    for (idx, (sender, (file, subs))) in std::iter::repeat(sender)
        .zip(media_files.iter().zip(subtitles.iter()))
        .enumerate()
    {
        if args.gen_audio() {
            let commands = generate_audio_commands(
                file,
                audio_files[idx].iter().map(|(a, b)| (*a, b.as_ref())),
                args.audio_stream_selector(),
            )?;
            audio_pb.inc_length(commands.len().try_into().unwrap());

            for command in commands {
                jobs.push(Job::Command {
                    pb: audio_pb.clone(),
                    command,
                });
            }
        }

        //jobs.extend(tmp.into_iter().map(Into::into));

        if args.gen_images() {
            let image_pb = multi.add(ProgressBar::new(subs.len().try_into().unwrap()));
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
                selector: args.video_stream_selector(),
                sender,
            });
        }

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

    if !args.no_media() {
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
    } else {
        trace!("not executing jobs because --no-media is specified");
    }

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

    if !args.no_deck() {
        package
            .write_to_file(args.package())
            .context("Failed to write package to file")?;
    } else {
        trace!("did not write an anki deck because --no-deck was specified");
    }

    if args.write_json() {
        let serialized = serde_json::to_string(&subtitles)?;
        print!("{}", serialized);
    }

    //read subtitles
    //filter/transform subtitles
    //generate media
    //generate deck
    Ok(())
}

fn main() -> Result<()> {
    setup_panic!();

    let args = Args::parse_from_env()?;

    let logger = pretty_env_logger::formatted_builder()
        .filter_level(args.verbosity())
        .build();

    if let Some(job_count) = args.job_count() {
        ThreadPoolBuilder::new()
            .num_threads(job_count)
            .build_global()
            .context("failed to initialize thread pool")?;
    }

    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init().unwrap();
    trace!("initialized logger");
    //execute

    libav::init().context("Failed to initialize libav")?;

    run(&args, multi.clone())?;
    /*
    if let Err(error) = run() {
        //print pretty error
    }*/
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ass::DialogueEvent;
    use crate::time::{Timespan, Timestamp};
    use assert_cmd::prelude::*;
    use serde::Deserialize;
    use std::process::Command;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize)]
    enum Dialogue {
        Text(String),
        Ass(DialogueEvent),
        Bitmap(String),
    }

    #[derive(Deserialize)]
    struct Subtitle {
        pub timespan: Timespan,
        pub diag: Dialogue,
    }

    #[derive(Deserialize)]
    struct SubtitleBundle {
        pub sub: Subtitle,
        pub sub_image: Option<String>,
        pub audio: Option<String>,
        pub image: Option<String>,
    }

    #[test]
    fn blacklist() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/sub.srt")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("-b")
            .arg("Hello")
            .assert()
            .success();
        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 0);
        Ok(())
    }

    #[test]
    fn blacklist_no_match() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/sub.srt")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("-b")
            .arg("don't match")
            .assert()
            .success();
        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 1);
        Ok(())
    }

    #[test]
    fn whitelist() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/sub.srt")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("-w")
            .arg("Hello")
            .assert()
            .success();
        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 1);
        Ok(())
    }

    #[test]
    fn whitelist_no_match() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/sub.srt")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("-w")
            .arg("don't match")
            .assert()
            .success();
        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 0);
        Ok(())
    }

    #[test]
    fn merge_subs() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/mergable_sub.srt")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("--merge")
            .assert()
            .success();
        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 2);

        assert_eq!(
            subs[0][1].sub.timespan.start(),
            Timestamp::from_millis(8000)
        );
        assert_eq!(subs[0][1].sub.timespan.end(), Timestamp::from_millis(9500));
        Ok(())
    }

    #[test]
    fn test_ass() -> TestResult {
        let out = Command::cargo_bin("stos")?
            .arg("tests/media/test.ass")
            .arg("--no-deck")
            .arg("--no-media")
            .arg("--write-json")
            .arg("--merge")
            .assert()
            .success();

        let stdout = String::from_utf8(out.get_output().stdout.clone())?;

        let subs: Vec<Vec<SubtitleBundle>> = serde_json::from_str(&stdout)?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].len(), 1);

        assert_eq!(subs[0][0].sub.timespan.start(), Timestamp::from_millis(0));
        assert_eq!(subs[0][0].sub.timespan.end(), Timestamp::from_millis(30050));
        Ok(())
    }
}
