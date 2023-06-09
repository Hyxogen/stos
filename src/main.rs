extern crate ffmpeg_next as ffmpeg;
extern crate pretty_env_logger;
mod subtitle;

use crate::subtitle::{Subtitle, SubtitleList};
use clap::Parser;
use log::{error, info, trace};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::Command;

use ffmpeg::{
    codec, codec::packet::packet::Packet, decoder, media, util::mathematics::rescale::Rescale,
    util::rational::Rational,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long = "media")]
    media_input: PathBuf,

    #[arg(long)]
    audio_index: Option<usize>,

    #[arg(short, long = "sub")]
    subtitle_input: Option<PathBuf>,

    #[arg(long)]
    sub_index: Option<usize>,

    #[arg(short, long)]
    coalesce: bool,
}

pub enum SubtitleError {
    NegativeStart,
    NegativeEnd,
    MissingTimestamp,
}

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &Packet,
) -> Result<Option<codec::subtitle::Subtitle>, ffmpeg::util::error::Error> {
    let mut subtitle = codec::subtitle::Subtitle::default();
    match decoder.decode(packet, &mut subtitle) {
        Ok(true) => Ok(Some(subtitle)),
        Ok(false) => Ok(None),
        Err(err) => Err(err),
    }
}

pub enum Rect {
    Text(String),
}

#[derive(Copy, Clone, Debug)]
pub struct Timestamp {
    ts: i64,
    time_base: Rational,
}

impl Timestamp {
    pub fn new(ts: i64, time_base: Rational) -> Self {
        Self { ts, time_base }
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ts = self.ts.rescale(self.time_base, Rational::new(1, 1000000));
        write!(
            f,
            "{}:{:02}:{}.{}",
            ts / (1000 * 1000 * 60 * 60),
            (ts / (1000 * 1000 * 60)) % 60,
            (ts / (1000 * 1000)) % 60,
            (ts / 1000) % 1000
        )
    }
}

/*
 *
 *
 * a: |---|
 * b:       |---|
 *
 *
 * a: |---|
 * b:  |---|
 *
 * a: |---|
 * b: |---|
 *
 * a:  |-|
 * b: |---|
 *
 * a:    |---|
 * b: |---|
 *
 * a:       |---|
 * b: |---|
 */

fn generate_command(subs: SubtitleList, audio_idx: Option<usize>) -> Command {
    let mut command = Command::new("ffmpeg");
    let mut idx = 0;

    for sub in subs.subs() {
        command
            .arg("-ss")
            .arg(Timestamp::new(sub.start, subs.time_base).to_string());
        command
            .arg("-to")
            .arg(Timestamp::new(sub.end, subs.time_base).to_string());
        command.arg("-map").arg(format!(
            "0:{}",
            audio_idx.map(|v| v.to_string()).unwrap_or("a".to_string())
        ));
        command.arg(format!("out{:03}.mka", idx));
        idx += 1;
    }
    command
}

fn main() {
    pretty_env_logger::init();
    let args = Args::parse();

    ffmpeg::init().unwrap();
    let sub_file = &args.subtitle_input.as_ref().unwrap_or(&args.media_input);
    let mut ictx = ffmpeg::format::input(sub_file).unwrap();

    let input_idx = match args.sub_index {
        None => {
            trace!("No subtitle stream index was selected, choosing first available one");

            match ictx.streams().best(media::Type::Subtitle) {
                Some(stream) => {
                    info!("Selected subtitle stream at index {}", stream.index());
                    stream.index()
                }
                None => {
                    error!(
                        "{}: No subtitle stream found",
                        sub_file.as_path().as_os_str().to_str().unwrap()
                    );
                    std::process::exit(1)
                }
            }
        }
        Some(sub_idx) => match ictx.streams().nth(sub_idx) {
            Some(stream) if stream.parameters().medium() == media::Type::Subtitle => stream.index(),
            Some(_) => {
                error!("Stream at index {} is not a subtitle stream", sub_idx);
                std::process::exit(1)
            }
            None => {
                error!(
                    "{} does not have a stream at index {}",
                    sub_file.as_path().as_os_str().to_str().unwrap(),
                    sub_idx
                );
                std::process::exit(1)
            }
        },
    };

    let context = codec::context::Context::from_parameters(
        ictx.streams()
            .find(|stream| stream.index() == input_idx)
            .unwrap()
            .parameters(),
    )
    .unwrap();

    let mut decoder = context.decoder().subtitle().unwrap();
    let mut subs = SubtitleList::new(ictx.streams().nth(input_idx).unwrap().time_base());

    for (stream, packet) in ictx.packets() {
        if stream.index() != input_idx {
            continue;
        }

        if let Ok(Some(subtitle)) = decode_subtitle(&mut decoder, &packet) {
            if let Ok(subtitle) = Subtitle::new(&subtitle, &packet, stream.time_base()) {
                subs.add_sub(subtitle);
            }
        }
    }

    if args.coalesce {
        subs = subs.coalesce();
    }

    let mut command = generate_command(subs, args.audio_index);
    //command.stdout(std::process::Stdio::null());
    //command.stderr(std::process::Stdio::null());
    command.arg("-i").arg(&args.media_input);
    command.arg("-loglevel").arg("warning");

    /*
    println!("ffmpeg \\");
    let mut idx = 0;
    for arg in command.get_args() {
        print!("{} ", arg.to_str().unwrap());
        idx = (idx + 1) % 7;
        if idx == 0 {
            println!("\\");
        }
    }*/
    //println!("{:?}", command.get_args().collect::<Vec<&std::ffi::OsStr>>());

    command.spawn().unwrap().wait().unwrap();
}
