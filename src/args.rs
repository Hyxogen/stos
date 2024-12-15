use crate::time::{Duration, Timestamp};
use crate::util::StreamSelector;
use anyhow::{bail, Context, Result};
use log::LevelFilter;
use rand::random;
use regex::Regex;
use std::ffi::OsString;
use std::path::PathBuf;

const DEFAULT_DECK_FILE: &str = "deck.apkg";
const DEFAULT_DECK_NAME: &str = "Stos Deck";
const DEFAULT_DECK_DESC: &str = "A deck generated by stos";
const DEFAULT_MERGE_DIST: i64 = 250;

fn print_help(executable: &str) {
    println!("USAGE:");
    println!(
        "    {} [OPTIONS] <SUBTITLE_FILE>... [-o <DECK>]",
        executable
    );
    println!(
        "    {} [OPTIONS] <SUBTITLE_FILE>... [-a | -i] [-m MEDIA_FILES...]",
        executable
    );
    println!("    {} -h | --help", executable);
    println!("    {} --version", executable);
    println!();
    println!("OPTIONS:");
    println!("    -h, --help                    Print this help message and exit");
    println!("    --version                     Print version and exit");
    println!("    -v                            Increase verbosity of program logs");
    println!("    -o FILE, --output=FILE        Specify the file to write the anki deck to [default: {}]", DEFAULT_DECK_FILE);
    println!("    -s INDEX, --sub-stream=INDEX  Select which stream to use from SUBTITLE_FILE as the subtitle stream");
    println!("    --sub-lang=LANGUAGE           Select which stream to use form SUBTITLE_FILE as the subtitle stream by language");
    println!("    --start TIMESTAMP             Specify from when the program should extract subtitles in hh:mm:ss format");
    println!("    --end TIMESTAMP               Specify until when the program should extract subtitles in hh:mm:ss format");
    println!("    --ignore-styled               Ignore subtitle texts that have been styled (only for ass format)");
    println!("    --merge                       Merge nearby subtitles that are the same into one. See `--max-dist`");
    println!("    --max-dist=MILLISECONDS       Used only with `--merge`. Will not merge subtitles that are more than MILLISECONDS apart [default: {}]", DEFAULT_MERGE_DIST);
    println!("    -a, --audio                   Generate audio snippets for the anki cards");
    println!("    --audio-stream=INDEX          Select which stream to use to generate the audio snippets");
    println!("    --audio-lang=LANGUAGE  Select which stream to use to generate the audio snippets by language");
    println!("    --pad-begin=MILLISECONDS      Pad the start time of each audio clip with MILLISECONDS amount");
    println!("    --pad-end=MILLISECONDS        Pad the end time of each audio clip with MILLISECONDS amount");
    println!("    --shift-audio=MILLISECONDS    Shift the audio timings by MILLISECONDS amount");
    println!("    --join-audio                  Join overlapping audio into one clip");
    println!("    -j JOBS, --jobs=JOBS          Specify amount of concurrent jobs stos will spawn [default: system logical core count]");
    println!("    -i, --image                   Generate images for the anki cards");
    println!("    --video-stream=INDEX          Select which stream to use to generate the images");
    println!("    -m, --media                   Specify media files from which to generate the audio snippets `-a` and/or images `-i`");
    println!("    --no-media                    Will not write media files specified by `-a` and/or `-i`");
    println!("    -b, --blacklist               Do not include subtitles that match this regex (can be used multiple times)");
    println!("    -w, --whitelist               Only include subtitles that match this regex (can be used multiple times)");
    println!("    --no-deck                     Do not write an anki deck package");
    println!(
        "    --id=ID                       Specify the id to give the anki deck [default: random]"
    );
    println!(
        "    --name=NAME                   Specify the name to give the anki deck [default: {}]",
        DEFAULT_DECK_NAME
    );
    println!("    --desc=DESC                   Specify the description to give the anki deck [default: {}]", DEFAULT_DECK_DESC);
}

#[derive(Clone, Debug)]
pub struct Args {
    program: String,

    sub_files: Vec<PathBuf>,
    sub_stream: Option<usize>,
    sub_lang: Option<String>,

    start: Timestamp,
    end: Timestamp,

    blacklist: Vec<Regex>,
    whitelist: Vec<Regex>,
    ignore_styled: bool,

    merge: bool,
    merge_diff: Duration,

    media_files: Vec<PathBuf>,

    gen_audio: bool,
    audio_stream: Option<usize>,
    audio_lang: Option<String>,
    pad_begin: Duration,
    pad_end: Duration,
    shift_audio: Duration,
    join_audio: bool,

    job_count: Option<usize>,

    gen_images: bool,
    video_stream: Option<usize>,
    image_width: Option<u32>,
    image_height: Option<u32>,

    no_media: bool,
    no_deck: bool,

    deck_id: i64,
    deck_name: String,
    deck_desc: String,
    package: PathBuf,

    write_json: bool,
    dump: bool,

    verbosity: LevelFilter,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            program: env!("CARGO_PKG_NAME").to_string(),
            sub_files: Default::default(),
            sub_stream: Default::default(),
            sub_lang: Default::default(),
            start: Timestamp::MIN,
            end: Timestamp::MAX,
            blacklist: Default::default(),
            whitelist: Default::default(),
            ignore_styled: true,
            merge: false,
            merge_diff: Duration::from_millis(DEFAULT_MERGE_DIST),
            media_files: Default::default(),
            gen_audio: false,
            audio_stream: Default::default(),
            audio_lang: Default::default(),
            pad_begin: Duration::from_millis(0),
            pad_end: Duration::from_millis(0),
            shift_audio: Duration::from_millis(0),
            join_audio: false,
            job_count: None,
            gen_images: false,
            video_stream: Default::default(),
            image_width: Default::default(),
            image_height: Default::default(),
            no_media: false,
            no_deck: false,
            deck_id: random(),
            deck_name: DEFAULT_DECK_NAME.to_string(),
            deck_desc: DEFAULT_DECK_DESC.to_string(),
            package: DEFAULT_DECK_FILE.into(),
            write_json: false,
            dump: false,
            verbosity: LevelFilter::Error,
        }
    }
}

impl Args {
    pub fn parse_from_env() -> Result<Self> {
        use lexopt::prelude::*;

        let mut args = Args::default();
        let mut parser = lexopt::Parser::from_env();

        let mut taking_media = false;

        if let Some(program) = parser.bin_name() {
            args.program = program.to_string();
        }

        while let Some(arg) = parser.next()? {
            match arg {
                Short('h') | Long("help") => {
                    print_help(args.program());
                    std::process::exit(0);
                }
                Long("version") => {
                    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                Short('m') | Long("media") => {
                    taking_media = true;
                }
                Short('s') | Long("sub-stream") => {
                    if args.sub_lang.is_some() {
                        eprintln!("--sub-stream and --sub-lang cannot be use at the same time");
                        std::process::exit(1);
                    }
                    args.sub_stream = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("sub-lang") => {
                    if args.sub_stream.is_some() {
                        eprintln!("--sub-stream and --sub-lang cannot be use at the same time");
                        std::process::exit(1);
                    }
                    args.sub_lang = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("start") => args.start = Self::convert(parser.value()?)?.parse()?,
                Long("end") => args.end = Self::convert(parser.value()?)?.parse()?,
                Short('b') | Long("blacklist") => {
                    let re = Self::convert(parser.value()?)?;
                    args.blacklist
                        .push(Regex::new(&re).context("Failed to compile regex for blacklist")?)
                }
                Short('w') | Long("whitelist") => {
                    let re = Self::convert(parser.value()?)?;
                    args.whitelist
                        .push(Regex::new(&re).context("Failed to compile regex for whitelist")?)
                }
                Long("ignore-styled") => {
                    args.ignore_styled = true;
                }
                Long("merge") => {
                    args.merge = true;
                }
                Long("max-dist") => {
                    args.merge_diff = Duration::from_millis(Self::convert_value(&mut parser)?)
                }
                Short('a') => {
                    args.gen_audio = true;
                }
                Long("audio-stream") => {
                    if args.audio_lang.is_some() {
                        eprintln!("--audio-stream and --audio-lang cannot be use at the same time");
                        std::process::exit(1);
                    }
                    args.audio_stream = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("audio-lang") => {
                    if args.audio_stream.is_some() {
                        eprintln!("--audio-stream and --audio-lang cannot be use at the same time");
                        std::process::exit(1);
                    }
                    args.audio_lang = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("pad-begin") => {
                    args.pad_begin = Duration::from_millis(Self::convert_value(&mut parser)?)
                }
                Long("pad-end") => {
                    args.pad_end = Duration::from_millis(Self::convert_value(&mut parser)?)
                }
                Long("shift-audio") => {
                    args.shift_audio = Duration::from_millis(Self::convert_value(&mut parser)?)
                }
                Long("join-audio") => {
                    args.join_audio = true;
                }
                Short('j') | Long("jobs") => {
                    args.job_count = Some(Self::convert(parser.value()?)?.parse()?);
                }
                Short('i') => {
                    args.gen_images = true;
                }
                Long("video-stream") => {
                    args.video_stream = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("no-media") => {
                    args.no_media = true;
                }
                Long("no-deck") => {
                    args.no_deck = true;
                }
                Long("id") => args.deck_id = Self::convert(parser.value()?)?.parse()?,
                Long("name") => args.deck_name = Self::convert(parser.value()?)?,
                Long("desc") | Long("description") => {
                    args.deck_desc = Self::convert(parser.value()?)?
                }
                Short('o') | Long("output") => {
                    args.package = Self::convert(parser.value()?)?.into()
                }
                Long("width") => args.image_width = Some(Self::convert(parser.value()?)?.parse()?),
                Long("height") => {
                    args.image_height = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Long("write-json") => {
                    args.write_json = true;
                }
                Long("dump") => {
                    args.dump = true;
                }
                Value(file) if taking_media => args.media_files.push(file.into()),
                Value(file) if !taking_media => args.sub_files.push(file.into()),
                Short('v') => {
                    args.verbosity = LevelFilter::Warn;

                    if let Some(val) = parser.optional_value() {
                        args.verbosity = match val.into_string().as_deref() {
                            Ok("v") => LevelFilter::Info,
                            Ok("vv") => LevelFilter::Debug,
                            Ok("vvv") => LevelFilter::Trace,
                            Ok(val) => {
                                eprintln!(
                                    "\"{}\" is not a valid value for the verbosity flag \"-v\"",
                                    val
                                );
                                std::process::exit(1);
                            }
                            Err(val) => {
                                eprintln!(
                                    "Failed to parse verbosity option: Invalid unicode: {}",
                                    val.to_string_lossy()
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Short(ch) => {
                    eprintln!("unknown short option `-{}`", ch);
                    std::process::exit(1);
                }
                Long(s) => {
                    eprintln!("unknown long  option `--{}`", s);
                    std::process::exit(1);
                }
                _ => todo!(),
            }
        }

        if args.sub_files.is_empty() {
            println!("The following argument was not provided:");
            println!("  <SUBTITLE_FILE>");
            println!();
            print_help(args.program());
            std::process::exit(0);
        }

        Ok(args)
    }

    fn convert(s: OsString) -> Result<String> {
        if let Ok(s) = s.into_string() {
            Ok(s)
        } else {
            bail!("could not convert string to utf8")
        }
    }

    fn convert_value<T: std::str::FromStr>(parser: &mut lexopt::Parser) -> Result<T>
    where
        <T as std::str::FromStr>::Err: std::error::Error + Sync + Send + 'static,
    {
        Ok(Self::convert(parser.value()?)?.parse::<T>()?)
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn sub_files(&self) -> &Vec<PathBuf> {
        &self.sub_files
    }

    pub fn sub_stream_selector(&self) -> StreamSelector {
        if let Some(stream_idx) = self.sub_stream {
            StreamSelector::Index(stream_idx)
        } else if let Some(sub_lang) = self.sub_lang.as_deref() {
            StreamSelector::Language(sub_lang)
        } else {
            StreamSelector::Best
        }
    }

    pub fn start(&self) -> Timestamp {
        self.start
    }

    pub fn end(&self) -> Timestamp {
        self.end
    }

    pub fn blacklist(&self) -> &Vec<Regex> {
        &self.blacklist
    }

    pub fn whitelist(&self) -> &Vec<Regex> {
        &self.whitelist
    }

    pub fn ignore_styled(&self) -> bool {
        self.ignore_styled
    }

    pub fn merge_subs(&self) -> bool {
        self.merge
    }

    pub fn merge_diff(&self) -> Duration {
        self.merge_diff
    }

    pub fn media_files(&self) -> &Vec<PathBuf> {
        &self.media_files
    }

    pub fn audio_stream_selector(&self) -> StreamSelector {
        if let Some(stream_idx) = self.audio_stream {
            StreamSelector::Index(stream_idx)
        } else if let Some(audio_lang) = self.audio_lang.as_deref() {
            StreamSelector::Language(audio_lang)
        } else {
            StreamSelector::Best
        }
    }

    pub fn gen_audio(&self) -> bool {
        self.gen_audio
    }

    pub fn pad_begin(&self) -> Duration {
        self.pad_begin
    }

    pub fn pad_end(&self) -> Duration {
        self.pad_end
    }

    pub fn shift_audio(&self) -> Duration {
        self.shift_audio
    }

    pub fn join_audio(&self) -> bool {
        self.join_audio
    }

    pub fn job_count(&self) -> Option<usize> {
        self.job_count
    }

    pub fn video_stream_selector(&self) -> StreamSelector {
        if let Some(stream_idx) = self.video_stream {
            StreamSelector::Index(stream_idx)
        } else {
            StreamSelector::Best
        }
    }

    pub fn gen_images(&self) -> bool {
        self.gen_images
    }

    pub fn no_media(&self) -> bool {
        self.no_media
    }

    pub fn no_deck(&self) -> bool {
        self.no_deck
    }

    pub fn deck_id(&self) -> i64 {
        self.deck_id
    }

    pub fn deck_name(&self) -> &str {
        &self.deck_name
    }

    pub fn deck_desc(&self) -> &str {
        &self.deck_desc
    }

    pub fn package(&self) -> &PathBuf {
        &self.package
    }

    pub fn write_json(&self) -> bool {
        self.write_json
    }

    pub fn dump(&self) -> bool {
        self.dump
    }

    pub fn verbosity(&self) -> LevelFilter {
        self.verbosity
    }
}
