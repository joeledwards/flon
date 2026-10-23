#[path = "../tests/support/large.rs"]
mod support;
use crate::{Config, ConfirmedOutput, InvalidCandidatePolicy, OutputMode, format_with_config};
use std::io::{Read, Write};
use support::{Corpus, Digest};

#[test]
fn seeded_large_valid_invalid_and_mixed_candidates() {
    for scenario in ["valid", "invalid", "mixed"] {
        let mut input = Vec::new();
        Corpus::new(42, 4, 16_384, scenario, 8192)
            .read_to_end(&mut input)
            .unwrap();
        // Independent expected output from the generated wire records. Only valid
        // candidates release their value; invalid candidates release nothing.
        let mut expected = Digest::default();
        for record in input.split_inclusive(|&b| b == b'\n') {
            if record.ends_with(b"\"}\n") {
                if expected.bytes == 0 {
                    expected.write_all(b"json = {};\n").unwrap();
                }
                expected.write_all(b"json.value = ").unwrap();
                expected.write_all(&record[9..record.len() - 2]).unwrap();
                expected.write_all(b";\n").unwrap();
            }
        }
        for threshold in [0, 4096, u64::MAX] {
            for chunk in [1, 4093, 8192] {
                let spool = tempfile::tempdir().unwrap();
                let output = format_with_config(
                    Corpus::new(42, 4, 16_384, scenario, chunk),
                    Digest::default(),
                    Config {
                        invalid_candidate: InvalidCandidatePolicy::Skip,
                        output_mode: OutputMode::Confirmed(ConfirmedOutput {
                            memory_threshold: threshold,
                            candidate_limit: None,
                            spool_directory: Some(spool.path().to_owned()),
                        }),
                        ..Config::default()
                    },
                )
                .unwrap();
                assert_eq!(
                    output, expected,
                    "{scenario}, threshold={threshold}, chunk={chunk}"
                );
                assert_eq!(spool.path().read_dir().unwrap().count(), 0);
            }
        }
    }
}

// Exercise the real spool write path without exhausting the host filesystem.
#[cfg(target_os = "linux")]
#[test]
fn disk_full_spool_write_preserves_os_error_and_releases_no_output() {
    use crate::{CandidateBuffer, Output};
    use std::fs::OpenOptions;
    for payload in [1, 65_536] {
        let mut output = Output::Confirmed {
            destination: Digest::default(),
            config: ConfirmedOutput::default(),
            candidate: CandidateBuffer::File {
                file: crate::SpoolFile::new(
                    OpenOptions::new().write(true).open("/dev/full").unwrap(),
                ),
                len: 0,
            },
        };
        let error = if payload == 1 {
            output.write_all(b"x").unwrap();
            output.commit_candidate().unwrap_err()
        } else {
            output.write_all(&vec![b'x'; payload]).unwrap_err()
        };
        assert_eq!(error.raw_os_error(), Some(28)); // ENOSPC on Linux
        assert_eq!(output.discard_candidate(), (0, payload != 1));
        assert_eq!(output.into_inner(), Digest::default());
    }
}

#[test]
fn large_spilled_candidate_cleans_up_on_truncation_limit_and_drop() {
    use crate::Formatter;
    use json_skimmer::Parser;
    let mut input = Vec::new();
    Corpus::new(73, 1, 32_768, "valid", 8192)
        .read_to_end(&mut input)
        .unwrap();
    // Remove closing quote, brace and newline: a large incomplete scalar.
    input.truncate(input.len() - 3);
    for limit in [None, Some(4096)] {
        let spool = tempfile::tempdir().unwrap();
        let config = Config {
            invalid_candidate: InvalidCandidatePolicy::Skip,
            output_mode: OutputMode::Confirmed(ConfirmedOutput {
                memory_threshold: 256,
                candidate_limit: limit,
                spool_directory: Some(spool.path().to_owned()),
            }),
            ..Config::default()
        };
        let mut visible = Digest::default();
        let result = format_with_config(input.as_slice(), &mut visible, config.clone());
        if limit.is_some() {
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::OutOfMemory);
        } else {
            result.unwrap();
        }
        assert_eq!(visible, Digest::default());
        assert_eq!(spool.path().read_dir().unwrap().count(), 0);
        if limit.is_none() {
            let mut formatter = Formatter::with_config(Digest::default(), config);
            Parser::default().push(&input, |event| formatter.event(event).unwrap());
            drop(formatter); // Caller abandons candidate without finish().
            assert_eq!(spool.path().read_dir().unwrap().count(), 0);
        }
    }
}

#[test]
fn confirmed_spill_batches_small_fragments_before_commit() {
    use crate::{CandidateBuffer, Output};
    let mut output = Output::new(
        Vec::new(),
        &OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 0,
            candidate_limit: None,
            spool_directory: None,
        }),
    );
    output.begin_candidate();
    for _ in 0..100 {
        output.write_all(b"abc").unwrap();
    }
    match &output {
        Output::Confirmed {
            candidate: CandidateBuffer::File { file, len },
            ..
        } => {
            assert_eq!(*len, 300);
            assert_eq!(
                file.metadata().unwrap().len(),
                0,
                "small writes should stay buffered"
            );
        }
        _ => panic!("expected spilled candidate"),
    }
    output.commit_candidate().unwrap();
    assert_eq!(output.into_inner(), b"abc".repeat(100));
}

#[test]
fn discarded_and_dropped_spills_do_not_flush_pending_bytes() {
    use crate::{CandidateBuffer, Output, SpoolFile};
    for discard in [true, false] {
        let file = tempfile::tempfile().unwrap();
        let inspection = file.try_clone().unwrap();
        let mut output = Output::Confirmed {
            destination: Vec::new(),
            config: ConfirmedOutput::default(),
            candidate: CandidateBuffer::File {
                file: SpoolFile::new(file),
                len: 0,
            },
        };
        output.write_all(b"unconfirmed").unwrap();
        if discard {
            assert_eq!(output.discard_candidate(), (11, true));
        }
        drop(output);
        assert_eq!(inspection.metadata().unwrap().len(), 0);
    }
}

#[test]
fn confirmed_buffer_boundaries_preserve_exact_output() {
    for size in [65_535, 65_536, 65_537] {
        let payload = "x".repeat(size);
        let input = format!("[\"{payload}\"]");
        let expected = format!("json = [];\njson[0] = \"{payload}\";\n");
        for memory_threshold in [0, 32] {
            let output = format_with_config(
                input.as_bytes(),
                Vec::new(),
                Config {
                    output_mode: OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold,
                        candidate_limit: Some(expected.len() as u64),
                        spool_directory: None,
                    }),
                    ..Config::default()
                },
            )
            .unwrap();
            assert_eq!(output, expected.as_bytes());
        }
    }
}
