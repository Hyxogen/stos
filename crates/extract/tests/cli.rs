use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn no_file() -> TestResult {
    Command::cargo_bin("extract")?
        .arg("a")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file"))
        .stdout(predicate::str::is_empty());
    Ok(())
}

#[test]
fn no_sub_stream() -> TestResult {
    Command::cargo_bin("extract")?
        .arg("tests/media/only_video.mp4")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a subtitle stream"))
        .stdout(predicate::str::is_empty());
    Ok(())
}

#[test]
fn no_subs() -> TestResult {
    Command::cargo_bin("extract")?
        .arg("tests/media/zero_length.srt")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::is_empty());
    Ok(())
}

#[test]
fn no_subtitle_at_index() -> TestResult {
    Command::cargo_bin("extract")?
        .arg("tests/media/sub.srt")
        .arg("-i")
        .arg("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not have a 2 streams"))
        .stdout(predicate::str::is_empty());
    Ok(())
}

#[test]
fn simple_subs() -> TestResult {
    Command::cargo_bin("extract")?
        .arg("tests/media/sub.srt")
        .assert()
        .success()
        .stdout(predicate::str::is_match("Hello World!::0:2500")?);
    Ok(())
}
