use anyhow::{bail, Result};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Args {
    program: String,

    sub_files: Vec<PathBuf>,
    sub_stream: Option<usize>,

    media_files: Vec<PathBuf>,

    gen_audio: bool,
    audio_stream: Option<usize>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            program: env!("CARGO_PKG_NAME").to_string(),
            sub_files: Default::default(),
            sub_stream: Default::default(),
            media_files: Default::default(),
            gen_audio: false,
            audio_stream: Default::default(),
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
                Short('m') | Long("media") => {
                    taking_media = true;
                }
                Short('s') | Long("sub-stream") => {
                    args.sub_stream = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Short('a') => {
                    args.gen_audio = true;
                }
                Long("audio-stream") => {
                    args.audio_stream = Some(Self::convert(parser.value()?)?.parse()?)
                }
                Value(file) if taking_media => args.media_files.push(file.into()),
                Value(file) if !taking_media => args.sub_files.push(file.into()),
                _ => todo!(),
            }
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

    /*
    pub fn program(&self) -> &str {
        &self.program
    }*/

    pub fn sub_files(&self) -> &Vec<PathBuf> {
        &self.sub_files
    }

    pub fn sub_stream(&self) -> Option<usize> {
        self.sub_stream
    }

    pub fn media_files(&self) -> &Vec<PathBuf> {
        &self.media_files
    }

    pub fn audio_stream(&self) -> Option<usize> {
        self.audio_stream
    }

    pub fn gen_audio(&self) -> bool {
        self.gen_audio
    }
}
