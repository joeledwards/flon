//! Explicit opt-in pre-release workload; metrics include generation and hashing.
#[path = "support/large.rs"]
mod support;
use flon::{Config, ConfirmedOutput, InvalidCandidatePolicy, OutputMode, format_with_config};
use std::io::{self, Read, Write};
use std::time::Instant;
use support::{Corpus, Digest};
struct Counted<R> {
    inner: R,
    bytes: u64,
}
impl<R: Read> Read for Counted<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(bytes)?;
        self.bytes += n as u64;
        Ok(n)
    }
}
fn setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .map(|s| s.parse().expect(name))
        .unwrap_or(default)
}
#[test]
#[ignore = "pre-release large-content correctness/performance; explicit opt-in"]
fn large_content_metrics() {
    let records = setting("FLON_RECORDS", 32);
    let tokens = setting("FLON_TOKENS", 262_144);
    let chunk = setting("FLON_CHUNK", 8192);
    let threshold = setting("FLON_SPILL_BYTES", 65536);
    let repeats = setting("FLON_REPEATS", 3);
    let seed = setting("FLON_SEED", 42) as u64;
    assert!(records > 0 && tokens > 0 && chunk > 0 && repeats > 0);
    assert!(
        threshold < tokens,
        "FLON_SPILL_BYTES must be below FLON_TOKENS to guarantee file spill"
    );
    let label = std::env::var("FLON_VERSION_LABEL")
        .expect("set FLON_VERSION_LABEL to revision/build identifier");
    assert!(
        label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
    );
    let metrics =
        std::env::var_os("FLON_METRICS").expect("set FLON_METRICS to an append-only JSONL path");
    let mut metrics = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(metrics)
        .unwrap();
    let parent = std::env::var_os("FLON_SPOOL_DIR");
    for scenario in ["valid", "invalid", "mixed"] {
        let mut reference = None;
        for mode in ["memory", "spill", "live"] {
            for iteration in 0..repeats {
                let spool = match &parent {
                    Some(path) => tempfile::tempdir_in(path).unwrap(),
                    None => tempfile::tempdir().unwrap(),
                };
                let output_mode = if mode == "live" {
                    OutputMode::Live
                } else {
                    OutputMode::Confirmed(ConfirmedOutput {
                        memory_threshold: if mode == "memory" {
                            u64::MAX
                        } else {
                            threshold as u64
                        },
                        candidate_limit: None,
                        spool_directory: Some(spool.path().to_owned()),
                    })
                };
                let mut input = Counted {
                    inner: Corpus::new(seed, records, tokens, scenario, chunk),
                    bytes: 0,
                };
                let start = Instant::now();
                let digest = format_with_config(
                    &mut input,
                    Digest::default(),
                    Config {
                        invalid_candidate: InvalidCandidatePolicy::Skip,
                        output_mode,
                        ..Config::default()
                    },
                )
                .unwrap();
                let elapsed = start.elapsed();
                assert_eq!(spool.path().read_dir().unwrap().count(), 0);
                if mode != "live" {
                    if let Some(expected) = &reference {
                        assert_eq!(&digest, expected);
                    } else {
                        reference = Some(digest.clone());
                    }
                    if scenario == "invalid" {
                        assert_eq!(digest, Digest::default());
                    }
                } else if scenario == "valid" {
                    assert_eq!(Some(&digest), reference.as_ref());
                }
                let row = format!(
                    "{{\"corpus\":1,\"version\":\"{label}\",\"package\":\"{}\",\"os\":\"{}\",\"arch\":\"{}\",\"scenario\":\"{scenario}\",\"mode\":\"{mode}\",\"seed\":{seed},\"records\":{records},\"tokens\":{tokens},\"chunk\":{chunk},\"spill_bytes\":{threshold},\"iteration\":{iteration},\"input_bytes\":{},\"output_bytes\":{},\"output_hash\":{},\"elapsed_ns\":{},\"input_mib_s\":{:.3}}}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    input.bytes,
                    digest.bytes,
                    digest.hash,
                    elapsed.as_nanos(),
                    input.bytes as f64 / 1048576.0 / elapsed.as_secs_f64()
                );
                writeln!(metrics, "{row}").unwrap();
                metrics.flush().unwrap();
                eprintln!("{row}");
            }
        }
    }
}
