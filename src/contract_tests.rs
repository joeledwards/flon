use super::*;

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
#[ignore = "Known bug: EOF error preservation is prioritized in ROADMAP.md; enable with the fix"]
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
    let mut parser = Parser::default();
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
            assert_eq!(render_chunks(input, config, &singles), expected.as_bytes());
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
