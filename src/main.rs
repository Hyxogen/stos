extern crate ffmpeg_next as libav;

use anyhow::{Result, Context};
use log::trace;

mod args;
mod subtitle;

use args::Args;

fn main() -> Result<()> {
    let args = Args::parse_from_env()?;

    pretty_env_logger::formatted_builder()
        .filter_level(args.verbosity)
        .init();
    trace!("initialized logger");

    libav::init().context("Failed to initialize libav")?;
    trace!("initialized libav");

    println!("sub files");
    for file in args.sub_files {
        println!("{:?}", file);
    }
    println!("media files");
    for file in args.media_files {
        println!("{:?}", file);
    }
    Ok(())
}
