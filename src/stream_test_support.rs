//! Shared contract checks for the library and CLI stream adapters.
use std::cell::Cell;
use std::io::{self, Write};
use std::rc::Rc;

use flon::{Config, ConfirmedOutput, Formatter, InvalidCandidatePolicy, OutputMode, Styling};
use json_skimmer::Parser;

// These are stream-adapter contracts shared by library and CLI unit tests.
pub(crate) fn check_depth_limits(mut run: impl FnMut(&[u8], Config) -> io::Result<Vec<u8>>) {
    assert_eq!(Config::default().max_depth.get(), 1000);
    for memory_threshold in [0, 4096] {
        for policy in [
            InvalidCandidatePolicy::Fail,
            InvalidCandidatePolicy::Skip,
            InvalidCandidatePolicy::SkipWithComment,
        ] {
            let config = Config {
                max_depth: std::num::NonZeroUsize::new(1).unwrap(),
                invalid_candidate: policy,
                output_mode: OutputMode::Confirmed(ConfirmedOutput {
                    memory_threshold,
                    ..ConfirmedOutput::default()
                }),
                ..Config::default()
            };
            let result = run(b"[[]] {\"ok\":true}", config);
            if policy == InvalidCandidatePolicy::Fail {
                let error = result.unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                assert_eq!(
                    error.to_string(),
                    "invalid JSON candidate: depth_limit_exceeded at byte 1 (candidate start: 0)"
                );
            } else {
                let mut expected = Vec::new();
                if policy == InvalidCandidatePolicy::SkipWithComment {
                    expected.extend_from_slice(
                        b"/* flon-error kind=depth_limit_exceeded offset=1 candidate_start=0 */\n",
                    );
                }
                expected.extend_from_slice(b"json = {};\njson.ok = true;\n");
                assert_eq!(result.unwrap(), expected);
            }
        }
    }
    let output = run(
        b"[[]] [2]",
        Config {
            max_depth: std::num::NonZeroUsize::new(1).unwrap(),
            invalid_candidate: InvalidCandidatePolicy::Skip,
            ..Config::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"json = [];\njson[0] = 2;\n");
}

pub(crate) struct FailOnceWriter {
    remaining: usize,
    failures: Rc<Cell<usize>>,
    writes_after_failure: Rc<Cell<usize>>,
    kind: io::ErrorKind,
}

impl Write for FailOnceWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failures.get() > 0 {
            self.writes_after_failure
                .set(self.writes_after_failure.get() + 1);
            return Ok(bytes.len());
        }
        if self.remaining == 0 {
            self.failures.set(1);
            return Err(io::Error::new(self.kind, "original EOF writer error"));
        }
        let count = self.remaining.min(bytes.len());
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn check_eof_writer_errors(
    mut run: impl FnMut(&[u8], FailOnceWriter, Config) -> io::Result<FailOnceWriter>,
) {
    for number in ["0", "-12", "1.50", "1e+3"] {
        let input = format!("[{number}");
        for policy in [
            InvalidCandidatePolicy::Fail,
            InvalidCandidatePolicy::Skip,
            InvalidCandidatePolicy::SkipWithComment,
        ] {
            for styling in [Styling::Plain, Styling::Ansi] {
                let config = Config {
                    invalid_candidate: policy,
                    styling,
                    ..Config::default()
                };
                // Derive only the setup byte budget from public events. The
                // expected error and no-further-writes contract are independent.
                let mut parser = Parser::default();
                let mut formatter = Formatter::with_config(Vec::new(), config.clone());
                parser.push(input.as_bytes(), |event| formatter.event(event).unwrap());
                let prefix_len = formatter.into_inner().len();
                for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
                    for extra in [0, 1] {
                        let failures = Rc::new(Cell::new(0));
                        let writes_after_failure = Rc::new(Cell::new(0));
                        let writer = FailOnceWriter {
                            remaining: prefix_len + extra,
                            failures: failures.clone(),
                            writes_after_failure: writes_after_failure.clone(),
                            kind,
                        };
                        let context =
                            format!("{number}, {policy:?}, {styling:?}, {kind:?}, extra {extra}");
                        let error = match run(input.as_bytes(), writer, config.clone()) {
                            Err(error) => error,
                            Ok(_) => panic!("swallowed error: {context}"),
                        };
                        assert_eq!(failures.get(), 1, "failure not reached: {context}");
                        assert_eq!(error.kind(), kind, "{context}");
                        assert_eq!(error.to_string(), "original EOF writer error", "{context}");
                        assert_eq!(
                            writes_after_failure.get(),
                            0,
                            "continued writing: {context}"
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn check_eof_policies(mut run: impl FnMut(&[u8], Config) -> io::Result<Vec<u8>>) {
    for output_mode in [
        OutputMode::Live,
        OutputMode::Confirmed(ConfirmedOutput::default()),
    ] {
        for policy in [
            InvalidCandidatePolicy::Fail,
            InvalidCandidatePolicy::Skip,
            InvalidCandidatePolicy::SkipWithComment,
        ] {
            let result = run(
                b"[1",
                Config {
                    output_mode: output_mode.clone(),
                    invalid_candidate: policy,
                    ..Config::default()
                },
            );
            if policy == InvalidCandidatePolicy::Fail {
                let error = result.unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                assert!(error.to_string().contains("unexpected_end"));
            } else {
                let bytes = result.unwrap();
                let mut expected = if output_mode == OutputMode::Live {
                    b"json = [];\njson[0] = 1;\n".to_vec()
                } else {
                    Vec::new()
                };
                if policy == InvalidCandidatePolicy::SkipWithComment {
                    expected.extend_from_slice(
                        b"/* flon-error kind=unexpected_end offset=2 candidate_start=0 */\n",
                    );
                }
                assert_eq!(bytes, expected);
            }
        }
    }
    // NumberEnd adds two bytes at EOF: a confirmed limit crossed there
    // must survive the subsequent invalid-candidate event for every policy.
    let prefix_len = b"json = [];\njson[0] = 1".len() as u64;
    for memory_threshold in [0, 4096] {
        for policy in [
            InvalidCandidatePolicy::Fail,
            InvalidCandidatePolicy::Skip,
            InvalidCandidatePolicy::SkipWithComment,
        ] {
            let error = run(
                b"[1",
                Config {
                    invalid_candidate: policy,
                    output_mode: OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold,
                        candidate_limit: Some(prefix_len + 1),
                        spool_directory: None,
                    }),
                    ..Config::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::OutOfMemory);
            assert_eq!(
                error.to_string(),
                "confirmed output exceeds configured candidate limit"
            );
        }
    }
}

/// Exercise both public Read adapters with identical byte-limit contracts.
pub(crate) fn check_candidate_limits(
    mut run: impl FnMut(&mut dyn std::io::Read, Config) -> io::Result<Vec<u8>>,
) {
    struct Chunks<'a> {
        bytes: &'a [u8],
        first: usize,
        rest: usize,
    }
    impl std::io::Read for Chunks<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let maximum = if self.first > 0 {
                std::mem::take(&mut self.first)
            } else {
                self.rest
            };
            let count = self.bytes.len().min(buffer.len()).min(maximum);
            buffer[..count].copy_from_slice(&self.bytes[..count]);
            self.bytes = &self.bytes[count..];
            Ok(count)
        }
    }
    for confirmed in [false, true] {
        for memory in [0, 4096] {
            let spool = tempfile::tempdir().unwrap();
            let base = Config {
                output_mode: if confirmed {
                    OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold: memory,
                        spool_directory: Some(spool.path().to_owned()),
                        ..ConfirmedOutput::default()
                    })
                } else {
                    OutputMode::Live
                },
                ..Config::default()
            };
            for input in [
                b"[]".as_slice(),
                b"[0]",
                b"[ ]",
                "{\"雪\":\"😀\"}".as_bytes(),
                br#"["\u1234",1e+2]"#,
            ] {
                let expected = run(&mut &input[..], base.clone()).unwrap();
                for maximum in [input.len() as u64, input.len() as u64 + 1] {
                    let mut cfg = base.clone();
                    cfg.max_candidate_bytes = std::num::NonZeroU64::new(maximum);
                    for split in 1..=input.len() {
                        for rest in [1, input.len()] {
                            assert_eq!(
                                run(
                                    &mut Chunks {
                                        bytes: input,
                                        first: split,
                                        rest
                                    },
                                    cfg.clone()
                                )
                                .unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
            let input = b"[1,2,3] [4]";
            for policy in [
                InvalidCandidatePolicy::Fail,
                InvalidCandidatePolicy::Skip,
                InvalidCandidatePolicy::SkipWithComment,
            ] {
                let mut cfg = base.clone();
                cfg.max_candidate_bytes = std::num::NonZeroU64::new(5);
                cfg.invalid_candidate = policy;
                for split in 1..=input.len() {
                    for rest in [1, input.len()] {
                        let result = run(
                            &mut Chunks {
                                bytes: input,
                                first: split,
                                rest,
                            },
                            cfg.clone(),
                        );
                        if policy == InvalidCandidatePolicy::Fail {
                            let error = result.unwrap_err();
                            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                            assert_eq!(
                                error.to_string(),
                                "invalid JSON candidate: candidate_limit_exceeded at byte 5 (candidate start: 0)"
                            );
                        } else {
                            let mut expected = Vec::new();
                            if !confirmed {
                                expected
                                    .extend_from_slice(b"json = [];\njson[0] = 1;\njson[1] = 2;\n");
                            }
                            if policy == InvalidCandidatePolicy::SkipWithComment {
                                expected.extend_from_slice(b"/* flon-error kind=candidate_limit_exceeded offset=5 candidate_start=0 */\n");
                            }
                            if confirmed {
                                expected.extend_from_slice(b"json = [];\n");
                            }
                            expected.extend_from_slice(b"json[0] = 4;\n");
                            assert_eq!(result.unwrap(), expected);
                        }
                        assert_eq!(spool.path().read_dir().unwrap().count(), 0);
                    }
                }
            }
            if confirmed {
                let mut cfg = base.clone();
                cfg.max_candidate_bytes = std::num::NonZeroU64::new(2);
                if let OutputMode::Confirmed(storage) = &mut cfg.output_mode {
                    storage.candidate_limit = Some(1);
                }
                assert_eq!(
                    run(&mut b"[]".as_slice(), cfg).unwrap_err().kind(),
                    io::ErrorKind::OutOfMemory
                );
            }
        }
    }
}
