//! Compiled CLI acceptance checks. Run separately from isolated unit tests.
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_flon"));
    for name in [
        "FLON_COLOR",
        "NO_COLOR",
        "FORCE_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
    ] {
        command.env_remove(name);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut command = command();
    command.args(args);
    execute(command, input)
}

fn execute(mut command: Command, input: &[u8]) -> Output {
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    // Inputs are deliberately tiny: neither input nor captured output fills a pipe.
    // Commands such as --show-config may close stdin before reading it.
    if let Err(error) = stdin.write_all(input) {
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
    drop(stdin);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("CLI failed to exit within ten seconds: {output:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_dash_file_and_buffering_have_exact_plain_output() {
    let input = r#"{"a":[1,"雪"],"empty":{}}"#.as_bytes();
    let expected =
        "json = {};\njson.a = [];\njson.a[0] = 1;\njson.a[1] = \"雪\";\njson.empty = {};\n"
            .as_bytes();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.json");
    std::fs::write(&path, input).unwrap();
    for source in [None, Some("-"), path.to_str()] {
        for buffering in ["buffered", "immediate"] {
            let mut args = vec!["--color=never", "--buffering", buffering];
            if let Some(source) = source {
                args.push(source);
            }
            let output = run(&args, input);
            assert!(output.status.success(), "{output:?}");
            assert_eq!(output.stdout, expected);
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
}

#[test]
fn confirmed_recovery_spills_discards_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let output = run(
        &[
            "--output-mode=confirmed",
            "--confirmed-memory=0",
            "--invalid=skip",
            "--diagnostics=quiet",
            "--summary",
            "--color=never",
            "--spool-directory",
            directory.path().to_str().unwrap(),
        ],
        br#"{"bad":"discard",} {"good":2}"#,
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"json = {};\njson.good = 2;\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("completed"), "{stderr}");
    assert!(stderr.contains("invalid"), "{stderr}");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn invalid_truncated_missing_input_and_invalid_options_fail() {
    for (args, input) in [
        (vec!["--output-mode=confirmed"], b"[1,]".as_slice()),
        (vec!["--output-mode=confirmed"], b"[\"truncated"),
        (vec!["--max-depth=0"], b"[]"),
        (vec!["--max-candidate-bytes=0"], b"[]"),
        (vec!["--buffering=immediate", "-B"], b"[]"),
        (vec!["--not-an-option"], b"[]"),
    ] {
        let output = run(&args, input);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(!output.stderr.is_empty(), "{output:?}");
    }
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.json");
    let output = run(&[missing.to_str().unwrap()], b"");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn environment_color_precedence_and_explicit_overrides() {
    for (environment, option, styled) in [
        (vec![], None, false),
        (vec![("FLON_COLOR", "always")], None, true),
        (vec![("FORCE_COLOR", "1"), ("NO_COLOR", "")], None, false),
        (
            vec![("FLON_COLOR", "always"), ("NO_COLOR", "1")],
            None,
            true,
        ),
        (vec![("FLON_COLOR", "always")], Some("--no-color"), false),
        (vec![("NO_COLOR", "1")], Some("--color=always"), true),
    ] {
        let mut command = command();
        for (name, value) in environment {
            command.env(name, value);
        }
        if let Some(option) = option {
            command.arg(option);
        }
        let output = execute(command, b"[1]");
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout.contains(&0x1b), styled, "{output:?}");
    }
}

#[test]
fn profile_options_override_defaults_and_show_config_does_not_read_input() {
    let output = run(
        &[
            "--profile=infinite",
            "--output-mode=live",
            "--no-summary",
            "--color=never",
            "--show-config",
        ],
        b"invalid",
    );
    assert!(output.status.success());
    let config = String::from_utf8(output.stdout).unwrap();
    assert!(config.contains("output_mode: Live"), "{config}");
    assert!(config.contains("summary: false"), "{config}");
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn closed_output_pipe_is_successful_early_termination() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    let (writer, reader) = UnixStream::pair().unwrap();
    drop(reader);
    let mut command = command();
    command.arg("--buffering=immediate");
    command.stdout(Stdio::from(OwnedFd::from(writer)));
    let output = execute(command, b"[1,2,3]");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
