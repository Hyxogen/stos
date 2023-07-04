use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn no_file() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/doesnt_exist.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file"));
    Ok(())
}

#[test]
fn no_sub_stream() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a subtitle stream"));
    Ok(())
}

#[test]
fn no_subs() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/zero_length.srt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("did not contain any subtitles"));
    Ok(())
}

#[test]
fn no_audio() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/sub.srt")
        .arg("-a")
        .arg("-m")
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a audio stream"));
    Ok(())
}

#[test]
fn no_audio_at_index() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/sub.srt")
        .arg("-a")
        .arg("--audio-stream")
        .arg("2")
        .arg("-m")
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a 2 streams"));
    Ok(())
}

#[test]
fn no_subtitle_at_index() -> TestResult {
    Command::cargo_bin("stos")?
        .arg("tests/media/sub.srt")
        .arg("--sub-stream")
        .arg("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a 2 streams"));
    Ok(())
}

#[test]
fn subs_only() -> TestResult {
    let dir = tempdir()?;
    let mut file = dir.path().to_path_buf();
    file.push("deck.apkg");
    Command::cargo_bin("stos")?
        .arg("tests/media/sub.srt")
        .arg("-o")
        .arg(&file)
        .assert()
        .success();
    assert!(file.exists());
    Ok(())
}

#[test]
fn subs_and_video() -> TestResult {
    let dir = tempdir()?;
    let mut file = dir.path().to_path_buf();
    file.push("image_0_0.jpg");
    Command::cargo_bin("stos")?
        .arg("tests/media/sub.srt")
        .arg("-i")
        .arg("-m")
        .arg("tests/media/only_video.mp4")
        .arg("--image-format")
        .arg(format!("{}/image_%f_%s.jpg", dir.path().to_string_lossy()))
        .assert()
        .success();
    assert!(file.exists());
    Ok(())
}
