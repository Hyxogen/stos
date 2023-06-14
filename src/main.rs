extern crate ffmpeg_next as ffmpeg;

mod subtitle;

use anyhow::{Context, Error, Result};
use clap::Parser;
use genanki_rs::{Deck, Field, Model, Note, Package, Template};
use glob::glob;
use log::{debug, error, info, trace, warn};
use rand::random;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use ffmpeg::{
    codec, decoder, media, util::mathematics::rescale::Rescale, util::rational::Rational, Stream,
};
use subtitle::*;

#[derive(Parser, Debug)]
struct Cli {
    sub_pattern: String,
    media_pattern: Option<String>,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,

    #[arg(long)]
    no_media: bool,

    #[arg(long)]
    no_deck: bool,

    #[arg(short, long)]
    sub_stream: Option<usize>,

    #[arg(short = 'a', long = "audio")]
    gen_audio: bool,

    #[arg(long)]
    audio_stream: Option<usize>,

    #[arg(long, default_value = "out_%f_%s.mka")]
    audio_format: String,

    #[arg(short = 'i', long = "image")]
    gen_image: bool,

    #[arg(long, default_value = "out_%f_%s.bmp")]
    image_format: String,

    #[arg(short, long, default_value = "deck.apkg")]
    output: String,

    #[arg(long = "id")]
    deck_id: Option<i64>,

    #[arg(short, long, default_value = "stos deck")]
    deck_name: String,

    #[arg(long, default_value = "")]
    deck_desc: String,

    #[arg(short, long)]
    coalesce: bool,

    #[arg(short, long = "print")]
    print_command: bool,
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

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for media::Type {
    fn name(&self) -> &'static str {
        match self {
            media::Type::Unknown => "unknown",
            media::Type::Video => "video",
            media::Type::Audio => "audio",
            media::Type::Data => "data",
            media::Type::Subtitle => "subtitle",
            media::Type::Attachment => "attachment",
        }
    }
}

fn get_stream<'a>(
    mut streams: impl Iterator<Item = Stream<'a>>,
    medium: media::Type,
    stream: Option<usize>,
) -> Result<usize> {
    match stream {
        Some(index) => {
            let stream = streams.nth(index).ok_or_else(|| {
                Error::msg(format!("The file does not contain {} stream(s)", index))
            })?;

            let stream_medium = stream.parameters().medium();

            if stream_medium == medium {
                Ok(index)
            } else {
                Err(Error::msg(format!(
                    "Incorrect stream type. Found {}, expected {}",
                    stream_medium.name(),
                    medium.name()
                )))
            }
        }
        None => Ok(streams
            .find(|stream| stream.parameters().medium() == medium)
            .ok_or_else(|| {
                Error::msg(format!(
                    "The file does not contain a {} stream",
                    medium.name()
                ))
            })?
            .index()),
    }
}

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &codec::packet::packet::Packet,
) -> Result<Option<codec::subtitle::Subtitle>> {
    let mut subtitle = Default::default();
    if decoder.decode(packet, &mut subtitle)? {
        Ok(Some(subtitle))
    } else {
        Ok(None)
    }
}

fn read_subtitles(
    mut ictx: ffmpeg::format::context::Input,
    stream_idx: usize,
) -> Result<SubtitleList> {
    let (context, time_base, codec) = {
        let stream = ictx.streams().nth(stream_idx).unwrap();
        let codec = stream.parameters().id();
        (
            codec::context::Context::from_parameters(stream.parameters())
                .with_context(|| format!("Failed to create codec context for {}", codec.name()))?,
            stream.time_base(),
            codec,
        )
    };
    debug!("created {} codec context for subtitle stream", codec.name());

    let mut decoder = context
        .decoder()
        .subtitle()
        .with_context(|| format!("Failed to open decoder for codec {}", codec.name()))?;
    debug!("opened decoder for {} codec", codec.name());

    let mut subs = SubtitleList::new(time_base);
    trace!("created subtitle list");

    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_idx {
            continue;
        }

        if let Some(subtitle) =
            decode_subtitle(&mut decoder, &packet).context("Failed to decode subtitle packet")?
        {
            match Subtitle::new(&subtitle, &packet, time_base) {
                Ok(subtitle) => {
                    subs.add_sub(subtitle);
                }
                Err(err) => {
                    warn!("Failed to convert a subtitle: {}", err);
                }
            }
        }
    }
    debug!("read {} subtitles", subs.len());

    Ok(subs)
}

fn get_subtitles(file: &PathBuf, stream: Option<usize>) -> Result<SubtitleList> {
    let file_str = file.to_string_lossy();

    let ictx =
        ffmpeg::format::input(file).with_context(|| format!("Failed to open {}", file_str))?;
    debug!("opened {} for reading subtitles", file_str);

    let stream_idx = get_stream(ictx.streams(), media::Type::Subtitle, stream)
        .with_context(|| format!("Failed to retrieve subtitle stream from {}", file_str))?;
    debug!(
        "{}: Using subtitle stream at index {}",
        file_str, stream_idx
    );

    trace!("{}: Reading subtitles...", file_str);
    read_subtitles(ictx, stream_idx)
        .with_context(|| format!("{}: Failed to read subtitles", file_str))
}

fn match_files(pattern: &str) -> Result<impl Iterator<Item = PathBuf>> {
    Ok(glob(pattern)
        .with_context(|| format!("Failed to match glob pattern \"{}\"", pattern))?
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!("Could not access file: {}", err);
                None
            }
        }))
}

fn create_note(
    model: &Model,
    index: usize,
    rect: &Rect,
    image_file: Option<&str>,
    audio_file: Option<&str>,
) -> Result<Note> {
    let audio_file = audio_file
        .map(|file| format!("[sound:{}]", file))
        .unwrap_or("".to_string());

    let image_file = image_file
        .map(|file| format!("<img src=\"{}\">", file))
        .unwrap_or("".to_string());

    match rect {
        Rect::Text(dialogue) => Ok(Note::new(
            model.clone(),
            vec![&index.to_string(), &image_file, &audio_file, dialogue],
        )
        .context("Failed to create Note")?),
    }
}

fn create_notes(
    subs: &Vec<SubtitleList>,
    image_fmt: Option<&str>,
    audio_fmt: Option<&str>,
) -> Result<Vec<Note>> {
    let model = Model::new(
        8815489913192057415,
        "Stos Model",
        vec![
            Field::new("Sequence indicator"),
            Field::new("Image"),
            Field::new("Audio"),
            Field::new("Text"),
        ],
        vec![Template::new("Card 1")
            .qfmt("{{Text}}")
            .afmt("{{FrontSide}}{{Image}}{{Audio}}{{Text}}")],
    );

    let mut notes = Vec::new();
    let mut idx = 0;

    for (file_idx, list) in subs.iter().enumerate() {
        for (sub_idx, sub) in list.iter().enumerate() {
            let values = FormatValues {
                file_idx,
                file_count: subs.len(),
                sub_idx,
                sub_count: list.len(),
            };
            for (_rect_idx, rect) in sub.iter().enumerate() {
                notes.push(create_note(
                    &model,
                    idx,
                    rect,
                    image_fmt.map(|fmt| format_filename(fmt, values)).as_deref(),
                    audio_fmt.map(|fmt| format_filename(fmt, values)).as_deref(),
                )?);
                idx += 1;
            }
        }
    }
    Ok(notes)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FormatValues {
    file_idx: usize,
    file_count: usize,
    sub_idx: usize,
    sub_count: usize,
}

fn format_filename(format: &str, values: FormatValues) -> String {
    let file_width: usize = values
        .file_count
        .checked_ilog10()
        .unwrap_or(1)
        .try_into()
        .unwrap_or(0usize)
        + 1;
    let sub_width: usize = values
        .sub_count
        .checked_ilog10()
        .unwrap_or(1)
        .try_into()
        .unwrap_or(0usize)
        + 1;
    format
        .replace("%f", &format!("{:0file_width$}", values.file_idx))
        .replace("%s", &format!("{:0sub_width$}", values.sub_idx))
}

fn generate_image_commands(
    subs: &Vec<SubtitleList>,
    media_files: &Vec<PathBuf>,
    image_fmt: &str,
) -> Vec<Command> {
    let mut commands = Vec::new();

    for (file_idx, (list, media_file)) in subs.iter().zip(media_files.iter()).enumerate() {
        for (sub_idx, sub) in list.iter().enumerate() {
            let values = FormatValues {
                file_idx,
                file_count: subs.len(),
                sub_idx,
                sub_count: list.len(),
            };

            let start = Timestamp::new(sub.start, list.time_base);
            let mut command = Command::new("ffmpeg");

            command.arg("-ss").arg(start.to_string());
            command.arg("-frames:v").arg("1");
            command.arg(format_filename(image_fmt, values));
            command.arg("-loglevel").arg("warning");
            command.arg("-i").arg(media_file);
            commands.push(command);
        }
    }
    commands
}

fn generate_audio_commands(
    subs: &Vec<SubtitleList>,
    audio_files: &Vec<PathBuf>,
    audio_stream: Option<usize>,
    audio_fmt: &str,
) -> Result<Vec<Command>> {
    let mut commands = Vec::new();

    for (file_idx, (list, audio_file)) in subs.iter().zip(audio_files.iter()).enumerate() {
        let stream_index = {
            let ictx = ffmpeg::format::input(audio_file)
                .with_context(|| format!("Failed to open {}", audio_file.to_string_lossy()))?;

            get_stream(ictx.streams(), media::Type::Audio, audio_stream)?
        };

        let mut command = Command::new("ffmpeg");
        for (sub_idx, sub) in list.iter().enumerate() {
            let values = FormatValues {
                file_idx,
                file_count: subs.len(),
                sub_idx,
                sub_count: list.len(),
            };

            let start = Timestamp::new(sub.start, list.time_base);
            let end = Timestamp::new(sub.end, list.time_base);

            command.arg("-ss").arg(start.to_string());
            command.arg("-to").arg(end.to_string());
            command.arg("-map").arg(format!("0:{}", stream_index));
            command.arg(format_filename(audio_fmt, values));
        }
        command.arg("-loglevel").arg("warning");
        command.arg("-i").arg(audio_file);
        commands.push(command);
    }
    Ok(commands)
}

/*
fn generate_command(
    file_idx: usize,
    file_count: usize,
    list: &SubtitleList,
    audio: Option<(&str, usize)>,
    image_fmt: Option<&str>,
) -> Command {
    let mut command = Command::new("ffmpeg");

    for (sub_idx, sub) in list.iter().enumerate() {
        let start = Timestamp::new(sub.start, list.time_base);

        command.arg("-ss").arg(start.to_string());
        let values = FormatValues {
            file_idx,
            file_count,
            sub_idx,
            sub_count: list.len(),
        };

        if let Some((audio_fmt, stream)) = audio {
            let end = Timestamp::new(sub.end, list.time_base);
            command.arg("-to").arg(end.to_string());
            command.arg("-map").arg(format!("0:{}", stream));
            command.arg(format_filename(audio_fmt, values));
        }

        if let Some(image_fmt) = image_fmt {
            command.arg("-frames:v").arg("1");
            command.arg(format_filename(image_fmt, values));
        }
    }

    command.arg("-loglevel").arg("warning");

    command
}*/

fn generate_package(
    deck_id: i64,
    deck_name: &str,
    deck_desc: &str,
    subs: &Vec<SubtitleList>,
    audio_fmt: Option<&str>,
    image_fmt: Option<&str>,
) -> Result<Package> {
    let notes = create_notes(subs, audio_fmt, image_fmt)?;
    trace!("created notes");

    let mut deck = Deck::new(deck_id, deck_name, deck_desc);
    for note in notes {
        deck.add_note(note);
    }

    let mut media = Vec::new();

    for (file_idx, list) in subs.iter().enumerate() {
        for (sub_idx, _) in list.iter().enumerate() {
            let values = FormatValues {
                file_idx,
                file_count: subs.len(),
                sub_idx,
                sub_count: list.len(),
            };
            if let Some(audio_fmt) = audio_fmt {
                media.push(format_filename(audio_fmt, values));
            }

            if let Some(image_fmt) = image_fmt {
                media.push(format_filename(image_fmt, values));
            }
        }
    }
    trace!("generated media references");

    Ok(Package::new(
        vec![deck],
        media.iter().map(|x| x.as_str()).collect(),
    )?)
}

fn main() -> Result<()> {
    let args = Cli::parse();

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbose.log_level_filter())
        .init();
    trace!("initialized logger");

    ffmpeg::init().context("Failed to initialize libav")?;
    trace!("initialized libav");

    let sub_files: Vec<PathBuf> = match_files(&args.sub_pattern)?.collect();
    debug!(
        "{} matched {} subtitle file(s)",
        args.sub_pattern,
        sub_files.len()
    );

    let subs = sub_files
        .iter()
        .map(|file| {
            get_subtitles(file, args.sub_stream).map(|list| {
                if args.coalesce {
                    let before = list.len();
                    let new_list = list.coalesce();
                    debug!(
                        "{}: coalesced {} subtitles into {}",
                        file.to_string_lossy(),
                        before,
                        new_list.len()
                    );
                    new_list
                } else {
                    list
                }
            })
        })
        .collect::<Result<Vec<SubtitleList>>>()?;

    let deck_id = args.deck_id.unwrap_or(random());
    debug!("using deck id {}", deck_id);

    let mut commands = Vec::new();

    if args.gen_audio || args.gen_image {
        let media_files: Vec<PathBuf> =
            match_files(args.media_pattern.as_ref().unwrap_or(&args.sub_pattern))?.collect();
        debug!(
            "{} matched {} media files",
            args.sub_pattern,
            media_files.len()
        );

        if media_files.len() != sub_files.len() {
            warn!("amount of subtitle ({}) does not match the amount of audio media ({}), will only convert {} files", sub_files.len(), media_files.len(), media_files.len().min(sub_files.len()));
        }

        if args.gen_audio {
            let audio_commands = generate_audio_commands(
                &subs,
                &media_files,
                args.audio_stream,
                &args.audio_format,
            )?;
            debug!(
                "generated {} command(s) to extract audio",
                audio_commands.len()
            );
            commands.extend(audio_commands);
        }

        if args.gen_image {
            let image_commands = generate_image_commands(&subs, &media_files, &args.image_format);
            debug!(
                "generated {} command(s) to extract images",
                image_commands.len()
            );
            commands.extend(image_commands);
        }
    }

    if args.print_command {
        for command in commands {
            println!("{:?}", command);
        }
        std::process::exit(1);
    }

    if args.print_command {
        std::process::exit(0);
    }

    if !args.no_media {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();

        pool.scope(|s| {
            for mut command in commands {
                s.spawn(move |_| match command.status() {
                    Ok(exitcode) => {
                        if exitcode.success() {
                            trace!("a ffmpeg command exited successfully");
                        } else {
                            error!("ffmepg exited with an error");
                        }
                    }
                    Err(err) => {
                        error!("failed to spawn command: {}", err);
                    }
                });
            }
        });
        debug!("done?");
    } else {
        debug!("did not execute ffmpeg commands because \"--no-media\" was specified");
    }

    if args.no_deck {
        debug!("did not create a anki package because \"--no-deck\" was specified");
    } else {
        let mut package = generate_package(
            deck_id,
            &args.deck_name,
            &args.deck_desc,
            &subs,
            if args.gen_image {
                Some(&args.image_format)
            } else {
                None
            },
            if args.gen_audio {
                Some(&args.audio_format)
            } else {
                None
            },
        )?;

        debug!("created package");
        package
            .write_to_file(&args.output)
            .context("failed to create anki deck package")?;
        info!("wrote package to {}", &args.output);
    }

    //Get subtitles
    //Generate deck
    //Generate media
    Ok(())
}
