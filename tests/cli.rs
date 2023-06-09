use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn no_audio_stream() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("--media")
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("stream found"));
    Ok(())
}
