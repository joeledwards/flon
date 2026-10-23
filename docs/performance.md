# Pre-release large-content checks

The default unit suite includes a fixed-seed randomized corpus of valid,
invalid (late syntax errors), and mixed candidates, each with 16,384 random
string tokens. Tokens include UTF-8, escapes, and ASCII. An independent
wire-record oracle checks output byte counts and hashes. Each workload runs
in memory, immediate spill, and memory-to-file transition modes, with one-byte,
4093-byte, and 8192-byte reads. Large truncated candidates, configured storage
limit errors, and abandoned formatters check cleanup too.

On Linux, a bounded `/dev/full` test exercises the internal file-backed write
path and checks the original ENOSPC error and no destination output. This does
not simulate failure during temporary-file creation or memory-to-file copying,
or establish full stream-adapter behavior on a genuinely full filesystem.
Empty-directory assertions do not prove descriptor release on systems that
unlink temporary files immediately. Crash cleanup and injected read/seek
failures remain gaps.

## Per-version checklist

Before each released version, run the checks below manually and append results
to the [performance results log](performance-results.md). This is a release
checklist, not an automatic CI job. Keep release gates open until required
cases are recorded; label exploratory or incomplete runs explicitly.

1. Run bounded unit tests with `../../scripts/with-rust cargo test --offline --lib --bins`.
2. Build FLON with `../../scripts/with-rust cargo build --offline --release --bin flon`.
   Build the companion generator with `../../scripts/with-rust cargo build --offline --release --manifest-path ../json-volcano/Cargo.toml --features cli`.
   Record the actual binary build revision, local modifications, compiler and
   flags, package version, and generator corpus version. A shell prompt's HEAD
   alone does not establish the executable's identity.
3. Record host/CPU/RAM/OS/kernel, input and spool filesystems/devices, cache
   conditions and background load. Capture Node.js and installed Node FLON
   versions when doing the optional Node comparison.
4. Use seed 42, depth 1, escape-percent 2 and unicode-percent 2 as the standard
   corpus parameters. First run the 100 MiB valid-record pipeline smoke test
   below. Measure generator-only throughput separately.
5. Run 1 GiB fixtures for records and giant-string shapes, both one candidate
   and 1,024 candidates. Measure live and unlimited confirmed forced-spill
   output (`--confirmed-memory 64KiB`); use an explicit spool directory and
   record its device. Compare memory-only confirmed storage on a 100 MiB
   fixture with `--confirmed-memory 512MiB --confirmed-limit unlimited`.
6. Before release, also run 10 GiB valid record/string fixtures with one and
   1,024 candidates in live and forced-spill modes. Record runtime and memory;
   these are manual opt-in runs, not ordinary unit tests.
7. Run the ignored legacy workload below for valid/invalid/mixed commit/discard
   checks and JSONL metrics. Separately check 1 GiB standalone-generator late
   invalid/truncated single candidates and mixed 1,024-candidate streams with
   confirmed `--invalid skip`: invalid/truncated candidates release nothing,
   mixed input retains only valid candidates, and spool cleanup follows exit.
   Record any resource failure and its exit status instead of a throughput
   success. Genuine ENOSPC remains a separate controlled manual check.
8. Use at least three measured repetitions per performance case; retain each
   raw result and report median and min/max. Keep input, color, summaries,
   destination and timing method identical across versions. Compare summary
   overhead as a separate case. Use file-input runs with generation completed
   beforehand to separate FLON throughput from generator backpressure.
9. Append a version entry with raw artifact links, measurements, correctness
   observations and missing cases/reasons. Record regressions for investigation;
   no universal timing threshold is established yet.

Before claiming equivalent-work speedups, compare Node and Rust output on a
small seeded fixture with color disabled and no ANSI escapes present. Check
assignment order/counts and exact bytes, then compare byte counts/hashes on a
larger fixture. Investigate number/escape differences against each contract;
do not silently normalize away missing or different values. This check is
still pending for the initial baseline. Include it in each comparator-version
change and record its result before interpreting performance ratios.

For the established pipeline smoke comparison, run from the repository root:

```bash
set -o pipefail
run_id=YYYY-MM-DD-version-build
run_dir="docs/performance-results/$run_id"
mkdir -p "$run_dir"

# Repeat this group three times; all timing and generator stderr is preserved.
{ time (
  ../json-volcano/target/release/json-volcano --bytes 100MiB --seed 42 --stats |
    ./target/release/flon --no-summary > /dev/null
); } 2>> "$run_dir/rust-live-100mib.log"

# Optional local Node comparison, using the same input and redirected defaults.
{ time (
  ../json-volcano/target/release/json-volcano --bytes 100MiB --seed 42 --stats |
    flon > /dev/null
); } 2>> "$run_dir/node-live-100mib.log"
```

Use shell `real` for the paired comparison, not the generator's slightly
shorter interval or terminal prompt overhead. Default redirected output is
plain in the observed builds; verify it for each comparator version and record
color environment overrides. Do not force color for just one implementation.
The Node reference CLI does not pass its `noColor` option through stdin, so
`--no-color` alone is not evidence of equivalent internal work.

Save the legacy harness JSONL into the same artifact directory, and write a
metadata file with the parameters above. For failed/manual runs capture exit
statuses alongside logs. The commands here are developer-run instructions;
editing this checklist does not execute benchmarks or authorize agents to
run integration tests.

## GiB-scale fixtures

Use the [bounded-memory standalone generator](json-generator.md) to prepare
1 GiB or 10 GiB fixtures before FLON timing. It supports one giant candidate,
many smaller candidates, record/giant-string shapes and late errors. Unlike
the legacy test corpus below, it never buffers a full candidate. Its generation
statistics are separate from FLON processing statistics. Both corpora retain
their own documented version-1 formats; their bytes are not interchangeable.

## Opt-in release workload

From the repository root, run manually before release:

```bash
FLON_VERSION_LABEL=revision-build-id \
FLON_METRICS=/absolute/path/flon-performance.jsonl \
../../scripts/with-rust cargo test --offline --release --test large_content \
  -- --ignored --nocapture --test-threads=1
```

This ignored test never runs in normal `cargo test`. It generates one candidate
at a time, not the entire dataset. Defaults: 32 candidates, 262,144 random tokens
per candidate, seed 42, 8192-byte reads, 65,536-byte spill threshold, three
iterations. Override with `FLON_RECORDS`, `FLON_TOKENS`, `FLON_SEED`, `FLON_CHUNK`,
`FLON_SPILL_BYTES`, and `FLON_REPEATS`. Token counts are not byte counts; UTF-8
and escapes have variable lengths. The spill threshold must be below the token count to guarantee disk overflow;
set `FLON_SPILL_BYTES=0` for immediate spill.
Set `FLON_SPOOL_DIR` to an existing directory on the filesystem to measure.
Memory is bounded by one generated candidate plus formatter storage; increase
tokens cautiously for the memory-only comparison.

Each valid/invalid/mixed dataset runs with unlimited confirmed storage in
memory, unlimited confirmed storage forced toward disk, and live output.
Confirmed modes must agree on output hashes; fully invalid confirmed input
must emit nothing; valid live output must match confirmed output. Every run
checks spool-directory cleanup. Recovery is silent so diagnostic formatting
costs do not obscure storage costs. These are library-path measurements;
they do not include the CLI infinite profile's comments, summary, or buffering.

JSONL rows append package version, required revision/build label, OS/architecture,
corpus version, seed, sizes, mode, iteration, actual input/output bytes, output
hash, elapsed nanoseconds, and input MiB/s. Keep raw rows for each release and
compare medians and spread with identical parameters. Timing includes random
generation and output hashing, excludes temporary-directory creation, and uses
a counting/hash sink rather than stdout. There is no fsync: disk measurements
include OS page-cache effects, not durable-storage throughput. No timing pass/fail
threshold is imposed.

Record alongside results: compiler version, build flags, CPU, RAM, OS/kernel,
filesystem, mount options, spool device, available space, and other system load.
Run on the same host/filesystem for version comparisons; report cold/warm-cache
conditions rather than treating cached writes as device speed. For process peak
RSS/CPU, wrap the command in `/usr/bin/time -v` where available (Cargo overhead
is included). The harness does not collect disk I/O counters or per-case RSS.

## CLI infinite profile and actual disk exhaustion

For a separately prepared large JSON fixture, the equivalent CLI comparison is:

```bash
../../scripts/with-rust cargo build --offline --release
./target/release/flon --profile infinite --color never --confirmed-memory 64KiB \
  --spool-directory /existing/spool/directory large.json > /dev/null
```

Check `--help` for options before running. Preserve stderr summaries with the
version and machine metadata. Repeat with a memory threshold above the fixture's
*generated output* size, and with `--output-mode live`. The infinite profile
means unlimited confirmed candidate output, not an endless benchmark. Use
bounded repeated runs for comparable measurements.

Test real ENOSPC only on a developer-provided disposable quota-limited
filesystem, never by filling the host disk. Verify nonzero exit, OS storage
error, no output for the failing candidate (earlier committed candidates may
remain), and spool cleanup after exit. Repeat failures at creation, initial
memory copy, and subsequent file writes if the environment permits controlling
available space. Filesystem setup and these integration runs are manual;
this harness neither provisions mounts nor runs disk-filling operations.
