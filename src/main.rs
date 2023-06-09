extern crate ffmpeg_next as ffmpeg;

use clap::Parser;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::process::Command;

use ffmpeg::{
    codec, codec::packet::packet::Packet, codec::subtitle, decoder, media,
    util::mathematics::rescale::Rescale, util::rational::Rational,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
}

pub enum SubtitleError {
    NegativeStart,
    NegativeEnd,
}

/*
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AssDialogueEvent {
    styled: bool,
    text: String,
}

pub enum ParseAssError {
    MissingText,
    UnbalancedBrackets,
}

impl FromStr for AssDialogueEvent {
    type Err = ParseAssError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut text = s.split(',').skip(10).peekable();

        if let Some(_) = text.peek() {
            let mut styled = false;
            let mut brackets = 0;

            let text = text.map(|part| {
                let mut unstyled_text = String::new();

                let mut chars = part.chars().peekable();
                while let Some(ch) = chars.next() {
                    if ch == '{' {
                        styled = true;
                        brackets += 1;
                    } else if ch == '}' {
                        brackets -= 1;
                    } else if brackets == 0 {

                        if ch == '\\' {
                            if let Some(next_ch) = chars.peek() {
                                if *next_ch == 'n' || *next_ch == 'N' {
                                    unstyled_text.push('\n');
                                    continue;
                                }
                            }
                        }

                        unstyled_text.push(ch);
                    }
                }
                unstyled_text
            }).collect();

            Ok(Self {
                styled,
                text,
            })
        } else {
            Err(ParseAssError::MissingText)
        }
    }
}
*/

fn decode_subtitle(
    decoder: &mut decoder::subtitle::Subtitle,
    packet: &Packet,
) -> Result<Option<subtitle::Subtitle>, ffmpeg::util::error::Error> {
    let mut subtitle = subtitle::Subtitle::default();
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
            "{}:{}:{}.{}",
            ts / (1000 * 1000 * 60 * 60),
            (ts / (1000 * 1000 * 60)) % 60,
            (ts / (1000 * 1000)) % 60,
            (ts / 1000) % 1000
        )
    }
}

pub struct Subtitle {
    rects: Vec<Rect>,
    pub start: i64,
    pub end: i64,
}

impl Subtitle {
    fn rects(&self) -> impl Iterator<Item = &Rect> {
        self.rects.iter()
    }
}

impl From<subtitle::Rect<'_>> for Rect {
    fn from(rect: subtitle::Rect) -> Self {
        match rect {
            subtitle::Rect::Text(text) => Rect::Text(text.get().to_string()),
            subtitle::Rect::Ass(ass) => Rect::Text(ass.get().to_string()),
            _ => todo!(),
        }
    }
}

impl Subtitle {
    pub fn new(subtitle: &subtitle::Subtitle, packet: &Packet) -> Result<Self, SubtitleError> {
        let start = match packet.pts() {
            Some(val) => val,
            None => subtitle.start().into(),
        };

        let end = start + packet.duration();

        Ok(Self {
            start: start.try_into().map_err(|_| SubtitleError::NegativeStart)?,
            end: end.try_into().map_err(|_| SubtitleError::NegativeEnd)?,
            rects: subtitle.rects().map(From::from).collect(),
        })
    }
}

pub struct SubtitleList {
    subs: Vec<Subtitle>,
    pub time_base: Rational,
}

impl SubtitleList {
    pub fn new(time_base: Rational) -> Self {
        Self {
            subs: Vec::default(),
            time_base,
        }
    }

    pub fn add_sub(&mut self, sub: Subtitle) -> &mut Self {
        self.subs.push(sub);
        self
    }

    pub fn subs(&self) -> impl Iterator<Item = &Subtitle> {
        self.subs.iter()
    }
}

fn generate_command(subs: SubtitleList) -> Command {
    let mut command = Command::new("ffmpeg");
    let mut idx = 0;

    for sub in subs.subs() {
        command
            .arg("-ss")
            .arg(Timestamp::new(sub.start, subs.time_base).to_string());
        command
            .arg("-to")
            .arg(Timestamp::new(sub.end, subs.time_base).to_string());
        command.arg("-map").arg("0:a");
        command.arg(format!("out{:03}.aac", idx));
        idx += 1;
    }
    command
}

fn main() {
    let args = Args::parse();

    ffmpeg::init().unwrap();
    let mut ictx = ffmpeg::format::input(&args.input).unwrap();
    let input_idx = ictx
        .streams()
        .best(media::Type::Subtitle)
        .expect("No subtitle stream found")
        .index();

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
            if let Ok(subtitle) = Subtitle::new(&subtitle, &packet) {
                subs.add_sub(subtitle);
            }
        }
    }

    let mut command = generate_command(subs);
    command.arg("-i").arg(&args.input);

    command.spawn().unwrap().wait().unwrap();
    println!("done");
}
