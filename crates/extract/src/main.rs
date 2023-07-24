extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};
use clap::Parser;
use log::{trace, debug};
use std::path::PathBuf;

mod ass;
mod subtitle;
mod time;

use subtitle::{read_subtitles, Rect, Subtitle};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'i', long = "index")]
    stream_idx: Option<usize>,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,

    files: Vec<PathBuf>,
}

fn print_escaped(s: &str) {
    for c in s.chars() {
        if c == ':' || c == '\\' {
            print!("\\");
        }
        print!("{}", c);
    }
}

fn print_rect(rect: &Rect) {
    match rect {
        Rect::Text(text) => {
            print_escaped(text);
            print!("::");
        }
        Rect::Ass(ass) => {
            print_escaped(&ass.text.dialogue);
            print!("::");
        }
        _ => todo!(),
    }
}

fn print_sub(sub: &Subtitle) {
    for rect in &sub.rects {
        print_rect(rect);
        println!("{}:{}", sub.start.as_millis(), sub.end.as_millis());
    }
}

//text:image:begin:end
fn main() -> Result<()> {
    let args = Args::parse();

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbose.log_level_filter())
        .init();

    libav::init().context("Failed to initialize libav")?;
    trace!("initialized ffmpeg");

    let mut first = true;
    for file in args.files.into_iter() {
        if !first {
            println!();
        }

        let subs = read_subtitles(&file, args.stream_idx)
            .with_context(|| format!("Failed to read subtitles from {}", file.to_string_lossy()))?;
        debug!("{}: Read {} subtitles", file.to_string_lossy(), subs.len());

        for sub in &subs {
            print_sub(&sub);
        }
        first = false;
    }
    Ok(())
}
