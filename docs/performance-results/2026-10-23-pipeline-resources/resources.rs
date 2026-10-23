//! Developer-run bounded pipeline heap probe; see docs/resources.md.
#[path = "support/allocation.rs"]
mod allocation;
use flon::{Config, ConfirmedOutput, Formatter, InvalidCandidatePolicy, OutputMode};
use json_skimmer::{Event, Mode, Parser};
use std::io::{self, Write};
use std::num::NonZeroUsize;

#[global_allocator]
static ALLOCATOR: allocation::Counting = allocation::Counting;

#[derive(Default)]
struct Counter(u64);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fixture(depth: usize, key: usize, scalar: usize) -> Vec<u8> {
    let mut input = Vec::new();
    for _ in 0..depth {
        input.extend_from_slice(b"{\"");
        input.extend(std::iter::repeat_n(b'k', key));
        input.extend_from_slice(b"\":");
    }
    input.push(b'"');
    let pattern = "é\\n".as_bytes();
    for _ in 0..scalar / pattern.len() {
        input.extend_from_slice(pattern);
    }
    input.extend(std::iter::repeat_n(b'x', scalar % pattern.len()));
    input.push(b'"');
    input.extend(std::iter::repeat_n(b'}', depth));
    input
}

#[derive(Debug)]
struct Report {
    completed: usize,
    invalid: usize,
    output: u64,
    warm: allocation::Snapshot,
    end: allocation::Snapshot,
    released: allocation::Snapshot,
}

fn probe(
    input: &[u8],
    depth: usize,
    key: usize,
    count: usize,
    chunk: usize,
    mode: &str,
) -> io::Result<Report> {
    // Directory ownership and its path allocation stay outside measured scope.
    let directory = tempfile::tempdir()?;
    let (result, released) = allocation::measure(|| {
        let output_mode = match mode {
            "live" => OutputMode::Live,
            "memory" | "spill" => OutputMode::Confirmed(ConfirmedOutput {
                memory_threshold: if mode == "spill" { 1024 } else { 1024 * 1024 },
                candidate_limit: Some(16 * 1024 * 1024),
                spool_directory: Some(directory.path().to_owned()),
            }),
            _ => unreachable!(),
        };
        let mut formatter = Formatter::with_config(
            Counter::default(),
            Config {
                max_depth: NonZeroUsize::new(depth).unwrap(),
                max_key_bytes: key,
                invalid_candidate: InvalidCandidatePolicy::Skip,
                output_mode,
                ..Config::default()
            },
        );
        let mut parser = Parser::with_max_depth(Mode::Scan, NonZeroUsize::new(depth).unwrap());
        let (mut completed, mut invalid) = (0, 0);
        let mut error = None;
        let mut consume = |event: Event<'_>| {
            match &event {
                Event::CandidateEnd { .. } => completed += 1,
                Event::Invalid(_) => invalid += 1,
                _ => {}
            }
            if error.is_none() {
                error = formatter.event(event).err();
            }
        };
        for part in input.chunks(chunk) {
            parser.push(part, &mut consume);
        }
        let warm = allocation::snapshot();
        for _ in 1..count {
            for part in input.chunks(chunk) {
                parser.push(part, &mut consume);
            }
        }
        parser.finish(&mut consume);
        if let Some(error) = error {
            return Err(error);
        }
        let end = allocation::snapshot();
        drop(parser);
        let output = formatter.into_inner().0;
        Ok((completed, invalid, output, warm, end))
    });
    let (completed, invalid, output, warm, end) = result?;
    if directory.path().read_dir()?.next().is_some() {
        return Err(io::Error::other("spool files remain after pipeline drop"));
    }
    Ok(Report {
        completed,
        invalid,
        output,
        warm,
        end,
        released,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7 {
        return Err("usage: resources DEPTH KEY_BYTES SCALAR_BYTES CANDIDATES CHUNK_BYTES live|memory|spill".into());
    }
    let values: Vec<usize> = args[1..6]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let (depth, key, scalar, count, chunk) =
        (values[0], values[1], values[2], values[3], values[4]);
    let mode = &args[6];
    if !(1..=1000).contains(&depth)
        || key == 0
        || key > 65536
        || scalar > 8 * 1024 * 1024
        || !(1..=100000).contains(&count)
        || !(1..=65536).contains(&chunk)
        || !["live", "memory", "spill"].contains(&mode.as_str())
    {
        return Err("arguments exceed probe bounds".into());
    }
    let size = depth * (key + 5) + scalar + 2;
    // Paths repeat keys in output, so bound generated work as well as input.
    let output_bound = (depth as u64 + 1) * (depth as u64 * (key as u64 + 4) + scalar as u64 + 32);
    if size * count > 16 * 1024 * 1024 || output_bound * count as u64 > 64 * 1024 * 1024 {
        return Err("case exceeds 16 MiB input or conservative 64 MiB output bound".into());
    }
    let input = fixture(depth, key, scalar);
    let report = probe(&input, depth, key, count, chunk, mode)?;
    println!(
        "{{\"depth\":{depth},\"key_bytes\":{key},\"scalar_bytes\":{scalar},\"candidates\":{count},\"chunk_bytes\":{chunk},\"mode\":\"{mode}\",\"input_bytes\":{},\"completed\":{},\"invalid\":{},\"output_bytes\":{},\"allocation_calls\":{},\"requested_bytes\":{},\"peak_live_bytes\":{},\"retained_after_first\":{},\"retained_at_end\":{},\"live_after_drop\":{}}}",
        input.len() * count,
        report.completed,
        report.invalid,
        report.output,
        report.end.calls,
        report.end.requested,
        report.end.peak,
        report.warm.live,
        report.end.live,
        report.released.live
    );
    if report.completed != count
        || report.invalid != 0
        || report.released.live != 0
        || report.released.accounting_error
        || report.end.live != report.warm.live
    {
        return Err("candidate, retention or cleanup check failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_modes_chunks_keys_and_scalars() {
        for (depth, key, scalar) in [(1, 1, 0), (8, 64, 9), (2, 4096, 8192)] {
            let input = fixture(depth, key, scalar);
            let mut output = None;
            for mode in ["live", "memory", "spill"] {
                for chunk in [1, 7, 8192] {
                    let r = probe(&input, depth, key, 4, chunk, mode).unwrap();
                    assert_eq!((r.completed, r.invalid), (4, 0));
                    assert_eq!(r.warm.live, r.end.live);
                    assert_eq!(r.released.live, 0);
                    assert!(!r.released.accounting_error);
                    if let Some(bytes) = output {
                        assert_eq!(r.output, bytes);
                    }
                    output = Some(r.output);
                }
            }
        }
    }
    #[test]
    fn live_scalar_heap_is_independent_of_token_size() {
        let mut peak = None;
        for size in [16, 4096, 16384] {
            let input = fixture(1, 1, size);
            for chunk in [1, 7, 8192] {
                let r = probe(&input, 1, 1, 2, chunk, "live").unwrap();
                if let Some(previous) = peak {
                    assert_eq!(r.end.peak, previous);
                }
                peak = Some(r.end.peak);
            }
        }
    }

    #[test]
    fn allocator_counts_growth_and_drop() {
        let (_, state) = allocation::measure(|| {
            let mut bytes = Vec::with_capacity(8);
            bytes.resize(4096, 1_u8);
            std::hint::black_box(&bytes);
            let live = allocation::snapshot();
            assert!(live.calls >= 2);
            assert!(live.live >= 4096);
            assert!(live.requested >= live.live);
            drop(bytes);
        });
        assert_eq!(state.live, 0);
        assert!(!state.accounting_error);
    }

    #[test]
    fn invalid_and_truncated_spills_release_memory_and_files() {
        let mut input = fixture(2, 64, 4096);
        input.truncate(input.len() - 1);
        for tail in [b"".as_slice(), b"!"] {
            let mut bytes = input.clone();
            bytes.extend_from_slice(tail);
            for chunk in [1, 7, 8192] {
                let r = probe(&bytes, 2, 64, 1, chunk, "spill").unwrap();
                assert_eq!(r.completed, 0);
                assert!(r.invalid > 0);
                assert_eq!(r.output, 0);
                assert_eq!(r.released.live, 0);
            }
        }
    }
}
