extern crate ffmpeg_next as libav;
use anyhow::Result;

mod subtitle;
mod time;

use subtitle::{read_subtitles_from_file, Dialogue};

fn run() -> Result<()> {
    let file = "a";
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
                    ass
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

fn main() {
    //parse args
    //execute
    if let Err(error) = run() {
        //print pretty error
    }
}
