use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

#[test]
fn cli_help_shows_usage() {
    let mut cmd = Command::cargo_bin("bilibili-dl").expect("binary exists");
    cmd.arg("--help");
    cmd.assert().success().stdout(contains("bilibili-dl")).stdout(contains("Usage"));
}

