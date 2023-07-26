extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, trace};
use std::path::PathBuf;

mod ass;
mod subtitle;
mod time;

use subtitle::{read_subtitles, Subtitle, SubtitleDialogue};

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

fn print_sub(sub: &Subtitle, idx: usize) -> Result<()> {
    match sub.diag() {
        SubtitleDialogue::Text(text) => {
            print_escaped(text);
            print!("::");
        }
        SubtitleDialogue::Ass(ass) => {
            print_escaped(&ass.text.dialogue);
            print!("::");
        }
        SubtitleDialogue::Bitmap(image) => {
            let file = format!("img_{:04}.jpg", idx);
            image.save(&file)?;
            print!(":{}:", file);
        }
    }
    println!("{}:{}", sub.start.as_millis(), sub.end.as_millis());
    Ok(())
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
            .with_context(|| format!("Failed to read subtitles from `{}`", file.to_string_lossy()))?;
        debug!("{}: Read {} subtitles", file.to_string_lossy(), subs.len());

        for (idx, sub) in subs.iter().enumerate() {
            print_sub(sub, idx)?;
        }
        first = false;
    }
    Ok(())
}
