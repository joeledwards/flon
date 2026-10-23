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
                file: OpenOptions::new().write(true).open("/dev/full").unwrap(),
                len: 0,
            },
        };
        let error = output.write_all(&vec![b'x'; payload]).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(28)); // ENOSPC on Linux
        assert_eq!(output.discard_candidate(), (0, true));
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
