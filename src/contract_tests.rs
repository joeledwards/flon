use super::*;

#[test]
fn stream_adapter_enforces_depth_and_recovers() {
    crate::stream_test_support::check_depth_limits(|input, config| {
        format_with_config(input, Vec::new(), config)
    });
    let input = b"[[]] {\"ok\":true}";
    let config = Config {
        max_depth: std::num::NonZeroUsize::new(1).unwrap(),
        invalid_candidate: InvalidCandidatePolicy::Skip,
        output_mode: OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 0,
            ..ConfirmedOutput::default()
        }),
        ..Config::default()
    };
    for split in 0..=input.len() {
        assert_eq!(
            render_chunks(input, config.clone(), &[split]),
            b"json = {};\njson.ok = true;\n"
        );
    }
}

#[test]
fn eof_policies_preserve_parse_and_confirmed_limit_errors() {
    crate::stream_test_support::check_eof_policies(|input, config| {
        format_with_config(input, Vec::new(), config)
    });
}

#[test]
fn eof_errors_stop_output_for_all_recovery_policies() {
    crate::stream_test_support::check_eof_writer_errors(|input, writer, config| {
        format_with_config(input, writer, config)
    });
}

#[test]
fn confirmed_output_discards_every_truncated_multibyte_prefix_at_eof() {
    for character in ['¢', '雪', '😀'] {
        let bytes = character.to_string().into_bytes();
        for length in 1..bytes.len() {
            for prefix in [b"{\"bad\":\"".as_slice(), b"{\""] {
                let input = [br#"{"ok":1}"#.as_slice(), prefix, &bytes[..length]].concat();
                for threshold in [0, 4096] {
                    let spool = tempfile::tempdir().unwrap();
                    let config = Config {
                        invalid_candidate: InvalidCandidatePolicy::Skip,
                        output_mode: OutputMode::Confirmed(ConfirmedOutput {
                            memory_threshold: threshold,
                            spool_directory: Some(spool.path().to_owned()),
                            ..ConfirmedOutput::default()
                        }),
                        ..Config::default()
                    };
                    for split in 0..=input.len() {
                        assert_eq!(
                            render_chunks(&input, config.clone(), &[split]),
                            b"json = {};\njson.ok = 1;\n",
                            "{character:?}, prefix {prefix:?}, length {length}, threshold {threshold}, split {split}"
                        );
                        assert_eq!(spool.path().read_dir().unwrap().count(), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn confirmed_utf8_recovery_never_releases_invalid_bytes() {
    let input = b"{\"bad\":\"\xed\xa0\x80\"} {\"ok\":\"\xf0\x9f\x98\x80\"}";
    let config = Config {
        invalid_candidate: InvalidCandidatePolicy::SkipWithComment,
        output_mode: OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 0,
            ..ConfirmedOutput::default()
        }),
        ..Config::default()
    };
    let expected = "\n/* flon-error kind=invalid_utf8 offset=9 candidate_start=0 */\njson = {};\njson.ok = \"😀\";\n";
    for split in 0..=input.len() {
        assert_eq!(
            render_chunks(input, config.clone(), &[split]),
            expected.as_bytes()
        );
    }
}

#[test]
fn eof_recovery_preserves_the_first_writer_error() {
    // The scalar prefix fits, but NumberEnd's semicolon at EOF fails.
    // The subsequent UnexpectedEnd recovery must not erase BrokenPipe.
    let writer = LimitedWriter {
        bytes: Vec::new(),
        budget: b"json = [];\njson[0] = 1".len(),
        error: io::ErrorKind::BrokenPipe,
    };
    let result = format_with_config(
        b"[1".as_slice(),
        writer,
        Config {
            invalid_candidate: InvalidCandidatePolicy::Skip,
            ..Config::default()
        },
    );
    match result {
        Err(error) => assert_eq!(error.kind(), io::ErrorKind::BrokenPipe),
        Ok(_) => panic!("EOF recovery swallowed the writer error"),
    }
}

fn render_chunks(input: &[u8], config: Config, cuts: &[usize]) -> Vec<u8> {
    let mut parser = Parser::with_max_depth(json_skimmer::Mode::Scan, config.max_depth);
    let mut formatter = Formatter::with_config(Vec::new(), config);
    let mut start = 0;
    for &end in cuts {
        parser.push(&input[start..end], |event| formatter.event(event).unwrap());
        start = end;
    }
    parser.push(&input[start..], |event| formatter.event(event).unwrap());
    parser.finish(|event| formatter.event(event).unwrap());
    formatter.into_inner()
}

#[test]
fn exact_output_corpus_is_independent_of_chunking_and_storage() {
    let cases: &[(&[u8], &str)] = &[
        (
            br#"{"":null,"$x":true,"a b":false,"a":1,"a":2}"#,
            "json = {};\njson[\"\"] = null;\njson.$x = true;\njson[\"a b\"] = false;\njson.a = 1;\njson.a = 2;\n",
        ),
        (
            br#"[123456789012345678901234567890,-0,1.200e+03]"#,
            "json = [];\njson[0] = 123456789012345678901234567890;\njson[1] = -0;\njson[2] = 1.200e+03;\n",
        ),
        (
            "{\"雪\":\"😀\",\"\\u0061\":\"\\n\\t\\\\\\\"\"}".as_bytes(),
            "json = {};\njson[\"雪\"] = \"😀\";\njson[\"\\u0061\"] = \"\\n\\t\\\\\\\"\";\n",
        ),
        (
            b"[[{},[]],null]",
            "json = [];\njson[0] = [];\njson[0][0] = {};\njson[0][1] = [];\njson[1] = null;\n",
        ),
    ];
    for &(input, expected) in cases {
        for output_mode in [
            OutputMode::Live,
            OutputMode::Confirmed(ConfirmedOutput {
                memory_threshold: 0,
                candidate_limit: Some(4096),
                spool_directory: None,
            }),
        ] {
            let config = Config {
                output_mode,
                ..Config::default()
            };
            for split in 0..=input.len() {
                assert_eq!(
                    render_chunks(input, config.clone(), &[split]),
                    expected.as_bytes(),
                    "{input:?}, split {split}"
                );
            }
            let singles: Vec<_> = (0..=input.len()).collect();
            assert_eq!(
                render_chunks(input, config.clone(), &singles),
                expected.as_bytes()
            );
            for cuts in crate::compatibility_tests::random_chunk_partitions(input.len()) {
                assert_eq!(
                    render_chunks(input, config.clone(), &cuts),
                    expected.as_bytes(),
                    "{input:?}, mode {:?}, cuts {cuts:?}",
                    config.output_mode
                );
            }
        }
    }
}

#[test]
fn confirmed_recovery_resets_paths_and_array_indexes() {
    let input = br#"[1] {"bad":["unfinished\q"]} [2,{"ok":3}]"#;
    let config = Config {
        invalid_candidate: InvalidCandidatePolicy::Skip,
        output_mode: OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 1,
            ..ConfirmedOutput::default()
        }),
        ..Config::default()
    };
    let expected = b"json = [];\njson[0] = 1;\njson[0] = 2;\njson[1] = {};\njson[1].ok = 3;\n";
    for split in 0..=input.len() {
        assert_eq!(render_chunks(input, config.clone(), &[split]), expected);
    }
}

#[derive(Debug)]
struct LimitedWriter {
    bytes: Vec<u8>,
    budget: usize,
    error: io::ErrorKind,
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() >= self.budget {
            return Err(io::Error::new(self.error, "test writer exhausted"));
        }
        let count = bytes.len().min(2).min(self.budget - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_writes_preserve_exact_output_and_failures_propagate() {
    let input = br#"{"a":[1,"two",true]}"#;
    let expected = format(input.as_slice(), Vec::new()).unwrap();
    for output_mode in [
        OutputMode::Live,
        OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 0,
            ..ConfirmedOutput::default()
        }),
    ] {
        for kind in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::WriteZero,
        ] {
            for budget in [0, 1, expected.len() - 1, expected.len()] {
                let writer = LimitedWriter {
                    bytes: Vec::new(),
                    budget,
                    error: kind,
                };
                let result = format_with_config(
                    input.as_slice(),
                    writer,
                    Config {
                        output_mode: output_mode.clone(),
                        ..Config::default()
                    },
                );
                match result {
                    Ok(writer) => {
                        assert_eq!(budget, expected.len());
                        assert_eq!(writer.bytes, expected);
                    }
                    Err(error) => {
                        assert!(budget < expected.len());
                        assert_eq!(error.kind(), kind);
                    }
                }
            }
        }
    }
}

#[test]
fn confirmed_limits_are_exact_and_reset_per_candidate() {
    let expected = b"json = {};\njson.a = 1;\n";
    for threshold in [0, expected.len() as u64, expected.len() as u64 + 1] {
        for limit in [expected.len() as u64 - 1, expected.len() as u64] {
            let result = format_with_config(
                br#"{"a":1}{"a":1}"#.as_slice(),
                Vec::new(),
                Config {
                    output_mode: OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold: threshold,
                        candidate_limit: Some(limit),
                        spool_directory: None,
                    }),
                    ..Config::default()
                },
            );
            if limit < expected.len() as u64 {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::OutOfMemory);
            } else {
                assert_eq!(result.unwrap(), b"json = {};\njson.a = 1;\njson.a = 1;\n");
            }
        }
    }
}

#[test]
fn spilled_truncated_candidate_is_discarded_at_eof() {
    let spool = tempfile::tempdir().unwrap();
    let output = format_with_config(
        br#"{"ok":1}{"bad":"unfinished"#.as_slice(),
        Vec::new(),
        Config {
            invalid_candidate: InvalidCandidatePolicy::Skip,
            output_mode: OutputMode::Confirmed(ConfirmedOutput {
                memory_threshold: 0,
                spool_directory: Some(spool.path().to_owned()),
                ..ConfirmedOutput::default()
            }),
            ..Config::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"json = {};\njson.ok = 1;\n");
    assert_eq!(spool.path().read_dir().unwrap().count(), 0);
}

#[test]
fn stream_adapter_enforces_candidate_limits() {
    crate::stream_test_support::check_candidate_limits(|input, config| {
        format_with_config(input, Vec::new(), config)
    });
}

#[test]
fn confirmed_spool_creation_failure_is_terminal_and_releases_no_output() {
    for policy in [
        InvalidCandidatePolicy::Fail,
        InvalidCandidatePolicy::Skip,
        InvalidCandidatePolicy::SkipWithComment,
    ] {
        for threshold in [0, b"json = {};\n".len() as u64] {
            let spool = tempfile::tempdir().unwrap();
            let missing = spool.path().join("missing");
            let mut visible = Vec::new();
            let error = format_with_config(
                br#"{"a":1} {"later":true}"#.as_slice(),
                &mut visible,
                Config {
                    invalid_candidate: policy,
                    output_mode: OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold: threshold,
                        spool_directory: Some(missing.clone()),
                        ..ConfirmedOutput::default()
                    }),
                    ..Config::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
            assert!(visible.is_empty());
            assert!(!missing.exists());
            assert_eq!(spool.path().read_dir().unwrap().count(), 0);
        }
    }
}

#[test]
fn confirmed_commit_failure_preserves_prefix_and_cleans_spool() {
    let expected = b"json = {};\njson.a = 1;\n";
    for policy in [
        InvalidCandidatePolicy::Fail,
        InvalidCandidatePolicy::Skip,
        InvalidCandidatePolicy::SkipWithComment,
    ] {
        for threshold in [0, 4096] {
            for budget in [0, 1, expected.len() - 1] {
                let spool = tempfile::tempdir().unwrap();
                let mut writer = LimitedWriter {
                    bytes: Vec::new(),
                    budget,
                    error: io::ErrorKind::PermissionDenied,
                };
                let error = format_with_config(
                    br#"{"a":1} {"later":true}"#.as_slice(),
                    &mut writer,
                    Config {
                        invalid_candidate: policy,
                        output_mode: OutputMode::Confirmed(ConfirmedOutput {
                            memory_threshold: threshold,
                            spool_directory: Some(spool.path().to_owned()),
                            ..ConfirmedOutput::default()
                        }),
                        ..Config::default()
                    },
                )
                .unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
                assert_eq!(writer.bytes, expected[..budget]);
                assert_eq!(spool.path().read_dir().unwrap().count(), 0);
            }
        }
    }
}

#[derive(Debug)]
struct ZeroAfterBudget {
    bytes: Vec<u8>,
    budget: usize,
    zero_responses: usize,
    writes_after_zero: usize,
}

impl Write for ZeroAfterBudget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.zero_responses != 0 {
            self.writes_after_zero += 1;
        }
        if self.bytes.len() == self.budget {
            self.zero_responses += 1;
            return Ok(0);
        }
        let count = bytes.len().min(2).min(self.budget - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn zero_length_destination_writes_are_terminal_and_clean_confirmed_storage() {
    // Keep a later candidate in the same read to catch writes after failure.
    // Any subsequent read is also an error in the adapter's stopping behavior.
    struct OneRead<'a>(Option<&'a [u8]>);
    impl Read for OneRead<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let input = self.0.take().expect("read continued after WriteZero");
            buffer[..input.len()].copy_from_slice(input);
            Ok(input.len())
        }
    }

    let input = br#"{"a":1}{"later":true}"#;
    let first_candidate_bytes = b"json = {};\njson.a = 1;\n".len();
    let expected = format(input.as_slice(), Vec::new()).unwrap();
    for policy in [
        InvalidCandidatePolicy::Fail,
        InvalidCandidatePolicy::Skip,
        InvalidCandidatePolicy::SkipWithComment,
    ] {
        for threshold in [None, Some(4096), Some(0)] {
            for budget in [
                0,
                1,
                first_candidate_bytes - 1,
                first_candidate_bytes,
                expected.len() - 1,
            ] {
                let spool = tempfile::tempdir().unwrap();
                let output_mode = threshold.map_or(OutputMode::Live, |memory_threshold| {
                    OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold,
                        spool_directory: Some(spool.path().to_owned()),
                        ..ConfirmedOutput::default()
                    })
                });
                let mut writer = ZeroAfterBudget {
                    bytes: Vec::new(),
                    budget,
                    zero_responses: 0,
                    writes_after_zero: 0,
                };
                let error = format_with_config(
                    OneRead(Some(input)),
                    &mut writer,
                    Config {
                        output_mode,
                        invalid_candidate: policy,
                        ..Config::default()
                    },
                )
                .unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WriteZero);
                assert_eq!(writer.bytes, expected[..budget]);
                assert_eq!(writer.zero_responses, 1);
                assert_eq!(writer.writes_after_zero, 0);
                assert_eq!(spool.path().read_dir().unwrap().count(), 0);
            }
        }
    }
}

#[test]
fn published_volcano_fixtures_commit_valid_candidates_and_discard_failures() {
    use json_volcano::{Config as FixtureConfig, Generator, Scenario, Shape};
    use std::io::Read;

    for scenario in [
        Scenario::Valid,
        Scenario::Invalid,
        Scenario::Mixed,
        Scenario::Truncated,
    ] {
        let candidates = if scenario == Scenario::Truncated {
            1
        } else {
            3
        };
        let mut input = Vec::new();
        Generator::new(FixtureConfig {
            bytes: 192,
            candidates,
            shape: Shape::String,
            scenario,
            escape_percent: 30,
            unicode_percent: 30,
            ..FixtureConfig::default()
        })
        .unwrap()
        .read_to_end(&mut input)
        .unwrap();
        // Independent assignment oracle from each complete valid wire string.
        // Preserve JSON escapes and UTF-8 exactly as they appear in the fixture.
        let mut expected = Vec::new();
        if scenario != Scenario::Truncated {
            for (index, candidate) in input.chunks(64).enumerate() {
                if scenario == Scenario::Valid || (scenario == Scenario::Mixed && index % 2 == 0) {
                    if expected.is_empty() {
                        expected.extend_from_slice(b"json = [];\n");
                    }
                    expected.extend_from_slice(b"json[0] = ");
                    expected.extend_from_slice(&candidate[1..candidate.len() - 2]);
                    expected.extend_from_slice(b";\n");
                }
            }
        }
        for threshold in [0, 4096] {
            let spool = tempfile::tempdir().unwrap();
            let config = Config {
                invalid_candidate: InvalidCandidatePolicy::Skip,
                output_mode: OutputMode::Confirmed(ConfirmedOutput {
                    memory_threshold: threshold,
                    spool_directory: Some(spool.path().to_owned()),
                    ..ConfirmedOutput::default()
                }),
                ..Config::default()
            };
            assert_eq!(
                format_with_config(input.as_slice(), Vec::new(), config.clone()).unwrap(),
                expected
            );
            for split in 0..=input.len() {
                assert_eq!(
                    render_chunks(&input, config.clone(), &[split]),
                    expected,
                    "{scenario:?}, threshold={threshold}, split={split}"
                );
            }
            assert_eq!(
                render_chunks(&input, config, &(0..=input.len()).collect::<Vec<_>>()),
                expected
            );
            assert_eq!(spool.path().read_dir().unwrap().count(), 0);
        }
    }
}
