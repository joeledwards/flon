# Bounded pipeline memory validation

The development-only `examples/resources.rs` measures requested heap allocation
on the calling thread while a public json-skimmer parser and FLON formatter
process synthetic JSON. It measures the combined pipeline, not formatter-only
allocations. Fixture creation, report formatting and temporary directory ownership
are outside the measured scope; formatter-owned spool paths and buffers are inside.
The plain counting writer retains no output. The helper delegates allocation to
`System` using a development-only unsafe allocator wrapper; runtime code is unchanged.

From the workspace root, the developer runs:

```bash
python3 scripts/measure-flon-resources \
  --toolchain-bin "$PWD/.tools/rust/rustup-home/toolchains/stable-x86_64-unknown-linux-gnu/bin"
```

The runner builds offline/locked in release mode and writes source snapshots,
compiler/host/revision/status/diff metadata, executable checksum, commands,
stdout/stderr/exit codes and incremental results.json under
`repos/flon/target/perf-resources-*`. The toolchain must already be installed.
The developer completed all 252 measurements on 2026-10-23. See the
[results log](performance-results.md#2026-10-23--bounded-pipeline-memory-validation)
for observed figures, retained artifacts and limits.

## Workload and interpretation

Four axes use nested objects containing a string with UTF-8 and raw JSON escapes:

- Depth 1, 16, 128, 1000 with one-byte keys.
- Eight simultaneously active keys, each 16 B, 4 KiB or 64 KiB.
- Scalar strings of 16 B, 64 KiB, 1 MiB or 8 MiB at depth one.
- 1, 100 or 1000 successive equal-shape candidates on one pipeline.

Each case runs with 8/64 KiB input chunks, three repetitions, and live,
confirmed-memory (1 MiB threshold), and confirmed-spill (1 KiB threshold)
configurations. These are storage policies: small cases need not spill and large
cases may spill with either confirmed threshold. Each confirmed candidate has a
16 MiB output limit. Spool files use a private temporary directory.
The probe bounds each invocation to 16 MiB input and a conservative 64 MiB
output estimate because deep/key-heavy paths expand output. It rejects oversized
arguments before creating the fixture. Repetitions may be set to 3..10.

Reports include allocation/reallocation calls, cumulative requested bytes,
peak live requested bytes, retained bytes after the first/final candidates,
post-drop live bytes, completed/invalid counts and generated output bytes.
The probe checks candidate counts, equal first/final retention, zero post-drop
live bytes, accounting consistency and absence of remaining spool files.
The runner also checks count-independent retention, scalar-size-independent
live-mode peak heap, and matching output byte counts across modes/chunks.
Repeated allocation of keys and spool storage is expected; cumulative allocation
calls need not plateau. Depth/key scaling and confirmed-storage peaks require
review of results.json. Equal byte counts do not establish byte-for-byte output
agreement; existing correctness fixtures and the separate reference gate cover that.

Requested heap bytes exclude allocator overhead, stack usage, process RSS,
filesystem/page-cache usage and other threads. This instrumented tool does not
measure throughput. Repeated synthetic candidates are not evidence of hours-long
stability. Large keys at depth 1000, crash/descriptor cleanup and total disk/RSS
remain outside this bounded campaign; existing storage tests cover error paths.
Do not close broader resource gates without recording these limits and reviewing
actual measurements.

## Isolated checks

The example's unit tests check modes/chunks, simultaneous keys, scalar-size
independence, repeated-candidate retention, allocator growth/drop, and malformed/
truncated spilled-candidate cleanup. Existing formatter contract tests cover
all two-chunk splits and exact bytes. Workspace reporting tests validate counts,
leak/growth rejection, comparisons and campaign bounds without running benchmarks.

```bash
# From repos/flon, using the installed compiler via the workspace wrapper:
../../scripts/with-rust cargo test --offline --locked --example resources
# From the workspace root:
python3 -m unittest discover -s scripts -p 'test_measure_flon_resources.py'
```
