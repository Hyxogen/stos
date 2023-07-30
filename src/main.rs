extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};

mod args;
mod ass;
mod subtitle;
mod time;

use args::Args;
use subtitle::{read_subtitles_from_file, Dialogue};

fn run(args: &Args) -> Result<()> {
    for file in args.sub_files() {
        for subtitle in read_subtitles_from_file(&file, None)? {
            match subtitle.dialogue() {
                Dialogue::Text(text) => {
                    println!(
                        "{}:{}:{}",
                        subtitle.start().as_millis(),
                        subtitle.end().as_millis(),
                        text
                    );
                }
                Dialogue::Ass(ass) => {
                    println!(
                        "{}:{}:{}",
                        subtitle.start().as_millis(),
                        subtitle.end().as_millis(),
                        ass.text.dialogue
                    );
                }
                Dialogue::Bitmap(_) => {
                    println!(
                        "{}:{}",
                        subtitle.start().as_millis(),
                        subtitle.end().as_millis(),
                    );
                }
            }
        }
    }
    //read subtitles
    //filter/transform subtitles
    //generate media
    //generate deck
    Ok(())
}

fn main() -> Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let args = Args::parse_from_env()?;
    //execute

    libav::init().context("Failed to initialize libav")?;

    run(&args)?;
    /*
    if let Err(error) = run() {
        //print pretty error
    }*/
    Ok(())
}
