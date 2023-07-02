use anyhow::Result;
use log::LevelFilter;
use rand::random;
use std::path::PathBuf;

pub struct Args {
    pub executable: String,
    pub sub_files: Vec<PathBuf>,
    pub sub_stream: Option<usize>,
    pub sub_format: String,

    pub coalesce: bool,

    pub media_files: Vec<PathBuf>,

    pub gen_audio: bool,
    pub audio_stream: Option<usize>,
    pub audio_format: String,

    pub gen_image: bool,
    pub image_format: String,

    pub no_media: bool,
    pub no_deck: bool,

    pub deck_id: i64,
    pub deck_name: String,
    pub deck_desc: String,
    pub package: PathBuf,

    pub verbosity: LevelFilter,
}

impl Default for Args { fn default() -> Self {
        Self {
            executable: "ffmpeg".to_string(),
            sub_files: Default::default(),
            sub_stream: Default::default(),
            sub_format: "sub_%f_%s_%r.jpg".to_string(),
            coalesce: false,
            media_files: Default::default(),
            gen_audio: false,
            audio_stream: Default::default(),
            audio_format: "out_%f_%s.mka".to_string(),
            gen_image: false,
            image_format: "out_%f_%s.jpg".to_string(),
            no_media: false,
            no_deck: false,
            deck_id: random(),
            deck_name: "Stos Deck".to_string(),
            deck_desc: "A deck generater by stos".to_string(),
            package: "deck.apkg".into(),
            verbosity: LevelFilter::Error,
        }
    }
}

impl Args {
    pub fn parse_from_env() -> Result<Self> {
        use lexopt::prelude::*;

        let mut args = Args::default();
        let mut subtitles = true;

        let mut parser = lexopt::Parser::from_env();

        if let Some(executable) = parser.bin_name() {
            args.executable = executable.to_string();
        }

        while let Some(arg) = parser.next()? {
            match arg {
                Short('m') | Long("media") => {
                    subtitles = false;
                }
                Short('s') | Long("sub-stream") => {
                    args.sub_stream = Some(parser.value()?.parse()?);
                }
                Short('a') | Long("audio") => {
                    args.gen_audio = true;
                }
                Long("audio-stream") => {
                    args.audio_stream = Some(parser.value()?.parse()?);
                }
                Long("audio-format") => {
                    args.audio_format = if let Ok(format) = parser.value()?.into_string() {
                        format
                    } else {
                        eprintln!("Failed to parse \"--audio-format\" option: Invalid unicode");
                        std::process::exit(1);
                    }
                }
                Short('i') | Long("image") => {
                    args.gen_image = true;
                }
                Long("image-format") => {
                    args.image_format = if let Ok(format) = parser.value()?.into_string() {
                        format
                    } else {
                        eprintln!("Failed to parse \"--image-format\" option: Invalid unicode");
                        std::process::exit(1);
                    }
                }
                Long("no-media") => {
                    args.no_media = true;
                }
                Long("no-deck") => {
                    args.no_deck = true;
                }
                Short('c') | Long("coalesce") => {
                    args.coalesce = true;
                }
                Short('o') | Long("output") => {
                    args.package = parser.value()?.into();
                }
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
                Value(val) if subtitles => {
                    args.sub_files.push(val.into());
                }
                Value(val) if !subtitles => {
                    args.media_files.push(val.into());
                }
                _ => panic!(),
            }
        }
        Ok(args)
    }
}
