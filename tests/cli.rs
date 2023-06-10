use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn no_audio_stream() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No audio stream present"));
    Ok(())
}

#[test]
fn zero_length_subtitle() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("tests/media/1000hz.mp3")
        .arg("tests/media/zero_length.srt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("did not contain any subtitles"));
    Ok(())
}

#[test]
fn invalid_audio_index() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("tests/media/1000hz.mp3")
        .arg("--audio-index")
        .arg("2")
        .arg("tests/media/zero_length.srt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Stream index 2 out of bounds"));
    Ok(())
}

#[test]
fn invalid_sub_index() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("stos")?
        .arg("tests/media/1000hz.mp3")
        .arg("tests/media/zero_length.srt")
        .arg("--sub-index")
        .arg("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Stream index 2 out of bounds"));
    Ok(())
}
