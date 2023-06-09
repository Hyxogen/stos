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

#[test]
fn zero_length_subtitle() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("--media")
        .arg("tests/media/1000hz.mp3")
        .arg("--sub")
        .arg("tests/media/zero_length.srt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no subtitles"));
    Ok(())
}

#[test]
fn invalid_audio_index() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("--media")
        .arg("tests/media/1000hz.mp3")
        .arg("--audio-index")
        .arg("2")
        .arg("--sub")
        .arg("tests/media/zero_length.srt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no stream at index 2"));
    Ok(())
}

#[test]
fn invalid_sub_index() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("--media")
        .arg("tests/media/1000hz.mp3")
        .arg("--sub")
        .arg("tests/media/zero_length.srt")
        .arg("--sub-index")
        .arg("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no stream at index 2"));
    Ok(())
}
