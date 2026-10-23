//! Shared contract checks for the library and CLI stream adapters.
use std::cell::Cell;
use std::io::{self, Write};
use std::rc::Rc;

use flon::{Config, ConfirmedOutput, Formatter, InvalidCandidatePolicy, OutputMode, Styling};
use json_skimmer::Parser;

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
