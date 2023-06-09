extern crate ffmpeg_next as ffmpeg;
extern crate pretty_env_logger;
mod subtitle;

use crate::subtitle::{Subtitle, SubtitleList};
use clap::Parser;
use log::{debug, error, trace, warn};
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

fn generate_command(subs: SubtitleList, audio_index: usize) -> Command {
    let mut command = Command::new("ffmpeg");

    for (idx, sub) in subs.subs().enumerate() {
        command
            .arg("-ss")
            .arg(Timestamp::new(sub.start, subs.time_base).to_string());
        command
            .arg("-to")
            .arg(Timestamp::new(sub.end, subs.time_base).to_string());
        command.arg("-map").arg(format!("0:{}", audio_index));
        command.arg(format!("out{:03}.mka", idx));
    }
    command
}

fn medium_to_string(medium: media::Type) -> &'static str {
    match medium {
        media::Type::Unknown => "unknown",
        media::Type::Video => "video",
        media::Type::Audio => "audio",
        media::Type::Data => "data",
        media::Type::Subtitle => "subtitle",
        media::Type::Attachment => "Attachment",
    }
}

fn get_stream(
    mut streams: ffmpeg::format::context::common::StreamIter,
    index: Option<usize>,
    medium: media::Type,
) -> Result<usize, String> {
    match index {
        None => match streams.best(medium) {
            Some(stream) => {
                debug!(
                    "No {} stream was specified, selected stream at index {}",
                    medium_to_string(medium),
                    stream.index()
                );
                Ok(stream.index())
            }
            None => Err(format!("No {} stream found", medium_to_string(medium))),
        },
        Some(index) => match streams.nth(index) {
            Some(stream) if stream.parameters().medium() == medium => Ok(stream.index()),
            Some(stream) => Err(format!(
                "Stream at index {} is not a {} stream. Found {} stream",
                index,
                medium_to_string(medium),
                medium_to_string(stream.parameters().medium())
            )),
            None => Err(format!("There is no stream at index {}", index)),
        },
    }
}

fn main() {
    pretty_env_logger::init();
    let args = Args::parse();

    ffmpeg::init().unwrap();
    let sub_file = &args.subtitle_input.as_ref().unwrap_or(&args.media_input);
    let mut sub_ictx = ffmpeg::format::input(sub_file).unwrap();
    let audio_ictx = ffmpeg::format::input(&args.media_input).unwrap();

    let input_index = match get_stream(sub_ictx.streams(), args.sub_index, media::Type::Subtitle) {
        Ok(index) => index,
        Err(error) => {
            error!(
                "{}: {}",
                sub_file.as_path().as_os_str().to_str().unwrap(),
                error
            );
            std::process::exit(1)
        }
    };

    let audio_index = match get_stream(audio_ictx.streams(), args.audio_index, media::Type::Audio) {
        Ok(index) => index,
        Err(error) => {
            error!(
                "{}: {}",
                args.media_input.as_path().as_os_str().to_str().unwrap(),
                error
            );
            std::process::exit(1)
        }
    };

    let context = codec::context::Context::from_parameters(
        sub_ictx
            .streams()
            .find(|stream| stream.index() == input_index)
            .unwrap()
            .parameters(),
    )
    .unwrap();

    let mut decoder = context.decoder().subtitle().unwrap();
    let mut subs = SubtitleList::new(sub_ictx.streams().nth(input_index).unwrap().time_base());

    for (stream, packet) in sub_ictx.packets() {
        if stream.index() != input_index {
            continue;
        }

        match decode_subtitle(&mut decoder, &packet) {
            Ok(Some(subtitle)) => match Subtitle::new(&subtitle, &packet, stream.time_base()) {
                Ok(subtitle) => {
                    subs.add_sub(subtitle);
                }
                Err(error) => {
                    warn!("Failed to convert subtitle: {}", error);
                }
            },
            Ok(None) => {
                trace!("Did not get subtitle this pass");
            }
            Err(error) => {
                error!("Failed to decode subtitle: {}", error);
                std::process::exit(1);
            }
        }
    }

    if subs.is_empty() {
        error!(
            "{} has no subtitles. The subtitle stream did exist",
            sub_file.as_path().as_os_str().to_str().unwrap()
        );
        std::process::exit(1);
    }

    if args.coalesce {
        subs = subs.coalesce();
    }

    let mut command = generate_command(subs, audio_index);
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
