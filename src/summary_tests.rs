use super::*;
use crate::{Cli, Diagnostics, resolve, write_stream_measured};
use clap::Parser as _;
use std::io::Read;

fn config(args: &[&str]) -> crate::EffectiveConfig {
    resolve(
        &Cli::try_parse_from(std::iter::once("flon").chain(args.iter().copied())).unwrap(),
        false,
    )
}

struct Chunks<'a> {
    bytes: &'a [u8],
    first: usize,
    chunk: usize,
}
impl Read for Chunks<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let limit = if self.first > 0 {
            std::mem::take(&mut self.first)
        } else {
            self.chunk
        };
        let count = limit.min(output.len()).min(self.bytes.len());
        output[..count].copy_from_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        Ok(count)
    }
}

#[test]
fn completed_structure_is_independent_of_chunks_and_output_mode() {
    let input = r#"noise {"a":["\u1234",-1.2e+3,true,false,null,{},[]],"é":"☃"} [1]"#;
    check_chunks(input.as_bytes());
}

fn check_chunks(input: &[u8]) {
    for mode in ["live", "confirmed"] {
        for split in 1..=input.len() {
            for chunk in [1, input.len()] {
                let mut output = Vec::new();
                let mut metrics = Metrics::default();
                write_stream_measured(
                    Chunks {
                        bytes: input,
                        first: split,
                        chunk,
                    },
                    &mut output,
                    config(&["--output-mode", mode, "--color", "never"]),
                    &mut metrics,
                )
                .unwrap();
                assert_eq!(metrics.input_bytes, input.len() as u64);
                assert_eq!(metrics.output_bytes, output.len() as u64);
                assert_eq!(
                    (metrics.started, metrics.completed, metrics.invalid),
                    (2, 2, 0)
                );
                assert_eq!(metrics.structure.kinds, [2, 3, 2, 2, 2, 1, 2]);
                assert_eq!(metrics.structure.depths, [2, 3, 7, 0, 0, 0]);
                assert_eq!(metrics.structure.max_depth, 2);
            }
        }
    }
}

#[test]
fn invalid_and_truncated_candidates_do_not_contribute_structure() {
    for input in [
        b"{\"bad\":[1,]} [true] {\"unfinished\":null".as_slice(),
        b"[\"\xff\"] [true] [",
    ] {
        for mode in ["live", "confirmed"] {
            for split in 1..=input.len() {
                let mut cfg = config(&["--invalid", "skip", "--output-mode", mode]);
                cfg.diagnostics = Diagnostics::Quiet;
                let mut metrics = Metrics::default();
                write_stream_measured(
                    Chunks {
                        bytes: input,
                        first: split,
                        chunk: 1,
                    },
                    Vec::new(),
                    cfg,
                    &mut metrics,
                )
                .unwrap();
                assert_eq!(metrics.completed, 1);
                assert_eq!(metrics.invalid, 2);
                assert_eq!(metrics.recovered, 2);
                assert_eq!(metrics.structure.kinds, [0, 1, 0, 0, 1, 0, 0]);
                assert_eq!(metrics.structure.depths, [1, 1, 0, 0, 0, 0]);
                assert_eq!(metrics.discarded_bytes > 0, mode == "confirmed");
            }
        }
    }
}

#[test]
fn histogram_aggregates_deeper_values_and_counts_suppressed_values() {
    let mut cfg = config(&["--max-key-bytes", "1", "--key-overflow", "skip"]);
    cfg.diagnostics = Diagnostics::Quiet;
    let mut metrics = Metrics::default();
    write_stream_measured(
        b"{\"long\":[[[[[null]]]]]}".as_slice(),
        Vec::new(),
        cfg,
        &mut metrics,
    )
    .unwrap();
    assert_eq!(metrics.structure.depths, [1, 1, 1, 1, 1, 2]);
    assert_eq!(metrics.structure.max_depth, 6);
    assert_eq!(metrics.structure.kinds, [1, 5, 0, 0, 0, 1, 1]);
}

#[test]
fn exact_plain_summary_and_styled_equivalence() {
    let metrics = Metrics {
        input_bytes: 20,
        output_bytes: 30,
        started: 2,
        completed: 1,
        invalid: 1,
        recovered: 1,
        ..Metrics::default()
    };
    let mut plain = Vec::new();
    write_summary(&mut plain, Duration::from_secs(2), &Ok(()), &metrics, false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&plain),
        "flon: completed in 2.000s\n  input: 20 bytes (10 bytes/s)\n  output: 30 bytes\n  candidates: 2 started, 1 completed, 1 invalid, 1 recovered\n  errors: 1 (1 recovered, 0 terminal)\n  values: 0 (completed candidates)\n    objects: 0  arrays: 0  strings: 0\n    numbers: 0  booleans: 0  nulls: 0  keys: 0\n  depth: max 0\n    0: 0  1: 0  2: 0  3: 0  4: 0  5+: 0\n  recovery: 0 confirmed-output bytes discarded\n"
    );
    let mut stripped = Vec::new();
    write_summary(
        anstream::AutoStream::new(&mut stripped, anstream::ColorChoice::Never),
        Duration::from_secs(2),
        &Ok(()),
        &metrics,
        true,
    )
    .unwrap();
    assert_eq!(stripped, plain);
    let mut styled = Vec::new();
    write_summary(&mut styled, Duration::ZERO, &Ok(()), &metrics, true).unwrap();
    assert!(styled.starts_with(b"\x1b["));
    assert!(styled.ends_with(b"\x1b[0m"));
}

struct Failure;
impl Write for Failure {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::PermissionDenied.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn summaries_preserve_processing_errors_and_disabled_is_silent() {
    for kind in [io::ErrorKind::InvalidData, io::ErrorKind::BrokenPipe] {
        let error = run(true, Failure, false, |_| {
            Err(io::Error::new(kind, "original"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.to_string(), "original");
        let mut text = Vec::new();
        run(true, &mut text, false, |_| Err(io::Error::from(kind))).unwrap_err();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains(if kind == io::ErrorKind::BrokenPipe {
            "pipe closed"
        } else {
            "failed"
        }));
        assert!(text.contains("errors: 1 (0 recovered, 1 terminal)"));
        assert!(!text.contains("NaN"));
    }
    assert_eq!(
        run(true, Failure, false, |_| Ok(())).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    let mut text = Vec::new();
    let mut calls = 0;
    run(false, &mut text, false, |_| {
        calls += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert!(text.is_empty());
}

#[test]
fn destination_count_handles_partial_writes_and_failures() {
    struct Budget(usize);
    impl Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let count = self.0.min(bytes.len()).min(2);
            self.0 -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for mode in ["live", "confirmed"] {
        let mut metrics = Metrics::default();
        let result = write_stream_measured(
            b"[1,2,3]".as_slice(),
            Budget(5),
            config(&["--output-mode", mode, "--buffering", "immediate"]),
            &mut metrics,
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(metrics.output_bytes, 5);
        assert_eq!(metrics.completed, 0);
    }
}

#[test]
fn input_flush_and_resource_failures_keep_observed_counts() {
    struct ReadFailure;
    impl Read for ReadFailure {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }
    let mut metrics = Metrics::default();
    let error =
        write_stream_measured(ReadFailure, Vec::new(), config(&[]), &mut metrics).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        (metrics.input_bytes, metrics.output_bytes, metrics.started),
        (0, 0, 0)
    );

    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::Other.into())
        }
    }
    let mut metrics = Metrics::default();
    let error = write_stream_measured(b"[]".as_slice(), FlushFailure, config(&[]), &mut metrics)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(metrics.output_bytes, b"json = [];\n".len() as u64);
    assert_eq!(metrics.completed, 1); // Candidate completion precedes final flush.

    for args in [
        vec!["--max-depth", "1"],
        vec!["--max-key-bytes", "1"],
        vec!["--output-mode", "confirmed", "--confirmed-limit", "1"],
    ] {
        let mut metrics = Metrics::default();
        assert!(
            write_stream_measured(
                b"{\"long\":[1]}".as_slice(),
                Vec::new(),
                config(&args),
                &mut metrics
            )
            .is_err()
        );
        assert_eq!(metrics.completed, 0);
        assert_eq!(metrics.structure, Structure::default());
    }
}

#[test]
fn output_bytes_include_forced_color_but_exclude_discarded_confirmed_output() {
    let mut lengths = Vec::new();
    for color in ["auto", "never", "always"] {
        let mut metrics = Metrics::default();
        let mut output = Vec::new();
        write_stream_measured(
            b"[1]".as_slice(),
            &mut output,
            config(&["--color", color]),
            &mut metrics,
        )
        .unwrap();
        assert_eq!(metrics.output_bytes, output.len() as u64);
        lengths.push(metrics.output_bytes);
    }
    assert_eq!(lengths[0], lengths[1]);
    assert!(lengths[2] > lengths[1]);
    for memory in ["0", "4096"] {
        let mut metrics = Metrics::default();
        let mut cfg = config(&[
            "--invalid",
            "skip",
            "--output-mode",
            "confirmed",
            "--confirmed-memory",
            memory,
        ]);
        cfg.diagnostics = Diagnostics::Quiet;
        write_stream_measured(b"[1,]".as_slice(), Vec::new(), cfg, &mut metrics).unwrap();
        assert_eq!(metrics.output_bytes, 0);
        assert!(metrics.discarded_bytes > 0);
        assert_eq!(metrics.structure, Structure::default());
    }
}

#[test]
fn summary_handles_partial_writes_empty_input_and_long_durations() {
    struct Partial(Vec<u8>);
    impl Write for Partial {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len().min(2);
            self.0.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for duration in [
        Duration::ZERO,
        Duration::from_millis(1234),
        Duration::from_secs(3601),
    ] {
        let mut expected = Vec::new();
        let mut partial = Partial(Vec::new());
        write_summary(&mut expected, duration, &Ok(()), &Metrics::default(), false).unwrap();
        write_summary(&mut partial, duration, &Ok(()), &Metrics::default(), false).unwrap();
        assert_eq!(expected, partial.0);
        let text = String::from_utf8(expected).unwrap();
        assert!(text.contains("input: 0 bytes (0 bytes/s)"));
        assert!(!text.contains("NaN"));
    }
}

#[test]
fn enabled_work_runs_once_and_summary_flush_failures_propagate() {
    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }
    let mut calls = 0;
    let error = run(true, FlushFailure, false, |_| {
        calls += 1;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(calls, 1);
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let error = run(true, FlushFailure, false, |_| {
        Err(io::ErrorKind::InvalidData.into())
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn summary_colors_distinguish_labels_numbers_separators_and_failures() {
    let mut output = Vec::new();
    write_summary(
        &mut output,
        Duration::from_millis(1234),
        &Ok(()),
        &Metrics::default(),
        true,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    let blue = anstyle::AnsiColor::Blue.on_default();
    let orange = anstyle::Ansi256Color(208).on_default();
    let white = anstyle::AnsiColor::White.on_default();
    assert!(text.contains(&format!("{blue}input{white}: {orange}0{white} bytes (")));
    assert!(text.contains(&format!("{orange}1.234{white}s\n")));
    // Histogram depths are labels, while their counts are numeric values.
    assert!(text.contains(&format!("{blue}5+{white}: {orange}0{white}")));
    assert!(text.contains(&format!(
        "{blue}objects{white}: {orange}0{white}  {blue}arrays"
    )));
    for kind in [io::ErrorKind::InvalidData, io::ErrorKind::BrokenPipe] {
        let mut output = Vec::new();
        write_summary(
            &mut output,
            Duration::ZERO,
            &Err(kind.into()),
            &Metrics::default(),
            true,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        let red = anstyle::AnsiColor::Red.on_default();
        let status = if kind == io::ErrorKind::BrokenPipe {
            "pipe closed"
        } else {
            "failed"
        };
        assert!(text.contains(&format!("{red}{status}{white}")));
    }
}

#[test]
fn candidate_limits_are_reported_without_committing_invalid_structure() {
    for mode in ["live", "confirmed"] {
        let mut cfg = config(&[
            "--max-candidate-bytes",
            "5",
            "--invalid",
            "skip",
            "--output-mode",
            mode,
        ]);
        cfg.diagnostics = Diagnostics::Quiet;
        let mut metrics = Metrics::default();
        write_stream_measured(b"[1,2,3] [4]".as_slice(), Vec::new(), cfg, &mut metrics).unwrap();
        assert_eq!(
            (
                metrics.started,
                metrics.completed,
                metrics.invalid,
                metrics.recovered
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(metrics.structure.kinds, [0, 1, 0, 1, 0, 0, 0]);
        assert_eq!(metrics.discarded_bytes > 0, mode == "confirmed");
    }
}
