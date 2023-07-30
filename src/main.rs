extern crate ffmpeg_next as libav;
use anyhow::{Context, Result};

mod ass;
mod subtitle;
mod time;

use subtitle::{read_subtitles_from_file, Dialogue};

fn run() -> Result<()> {
    let file = "lyco.srt";
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
            Dialogue::Bitmap(image) => {
                println!(
                    "{}:{}:{}",
                    subtitle.start().as_millis(),
                    subtitle.end().as_millis(),
                    image
                );
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
    //parse args
    //execute

    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    libav::init().context("Failed to initialize libav")?;

    run()?;
    /*
    if let Err(error) = run() {
        //print pretty error
    }*/
    Ok(())
}
