# Performance results log

Append an entry for every released version; do not replace older measurements.
Follow the [performance checklist](performance.md#per-version-checklist).
Retain raw timing output and machine/build metadata under
`docs/performance-results/<date>-<version>-<build>/` and link it from the entry.
Only commit results/metadata, not generated GiB-scale fixtures. Never present
single runs as medians or an unverified shell revision as the binary revision.

The generator has since moved to the independent sibling `json-volcano` project.
Historical executable names below are intentional. Output equivalence between
Node and Rust has not yet been verified; the 8.23× figure is a measured wall-time
ratio, not yet a validated equivalent-work speedup.

## 2026-10-23 — initial developer measurements on hulk

Status: preliminary baseline, not a completed release benchmark. Results below
were run by the developer and supplied in the conversation; the agent did not
execute these benchmarks. Raw artifacts were not supplied for these CLI runs,
so the reported values are transcribed here.

Build identity: shell prompt displayed `flon:main:d1e99db`; the revision of the
executed binary and working-tree state were not independently verified. Rust
package, compiler, and installed Node FLON/Node.js versions were not reported.
Host: `hulk`; CPU, RAM, OS/kernel, filesystem, device, cache state, and system
load were not recorded. Rust executables came from `target/release`.

All inputs used standalone generator corpus 1, seed 42, one root candidate,
depth 1, records shape, valid scenario, escape-percent 2, unicode-percent 2.
Stdout was redirected to `/dev/null`. Measurements are single observations.
MiB means 1,048,576 bytes. Pipeline elapsed times include both processes.

| Workload | Input bytes | Wall time (s) | Input MiB/s | Notes |
| --- | ---: | ---: | ---: | --- |
| Generator alone | 1,073,741,824 | 2.047113 | 500.217 | Generator's own elapsed time |
| Generator → Rust live, summary enabled | 1,073,741,824 | 25.210 | 40.62 | FLON summary time; shell `real` not captured |
| Generator → Rust live, summary disabled | 104,857,600 | 2.492 | 40.13 | Shell `real`; generator reported 40.182 MiB/s |
| Generator → Node FLON, defaults | 104,857,600 | 20.520 | 4.87 | Shell `real`; generator reported 4.883 MiB/s |

The paired 100 MiB runs show approximately **8.23×** shorter wall time for
Rust. This is a pipeline/default-behavior comparison, not a claim about all
JSON workloads or equivalent internal color handling. Node emitted no ANSI
escapes when piped in the developer's check; Rust auto color also resolves to
plain for redirected output. Node may still call color helpers internally.

Exact commands for the paired observations (with `set -o pipefail`):

```bash
time (
  ./target/release/flon-json-gen --bytes 100MiB --stats |
    ./target/release/flon --no-summary > /dev/null
)

time (
  ./target/release/flon-json-gen --bytes 100MiB --stats |
    flon > /dev/null
)
```

Captured shell CPU times: Rust `user=2.712s`, `sys=0.045s`; Node
`user=21.060s`, `sys=0.258s`. Generator statistics reported 2.488704s for the
Rust pipeline and 20.478833s for Node. These intervals differ slightly from
shell `real`, so the comparison uses shell `real` consistently.

The 1 GiB Rust summary reported 1,830,393,288 output bytes, one candidate
started/completed, no errors/recovery, 49,246,334 values, 8,207,722 objects,
41,038,610 keys, and maximum value depth 2. Its command used
`--color never --summary`. Disabling summaries on the later 100 MiB run did
not yield a clear throughput improvement; repeated isolated runs are needed
to quantify summary overhead.

Outstanding for this baseline: repeated runs/median/spread; verified build and
host metadata; file-input isolation; memory/RSS; forced-spill measurements;
giant strings, many candidates, malformed recovery, and 10 GiB runs. Earlier
~15 MiB results from the legacy generated/hash-sink harness are a different
workload and must not be combined with this table.

## 2026-10-23 — 1.0.0 live file-input baseline

Developer-run via workspace `scripts/measure-flon-baseline`; all build,
generation and measurement exit statuses were zero. [Raw artifacts and metadata](performance-results/2026-10-23-1.0.0-1cd5d96-live-baseline/metadata.txt)
are retained alongside commands, timings, executable/input SHA-256 hashes,
manifests and lockfile; the generated 100 MiB fixture is excluded.

FLON revision `1cd5d96790d9ce678e69404abacf155fae5da205`, local parser
`a9f573738ed17c4c2859b69f26f7d95ebadabca8`; both working trees were clean.
Offline locked release build, Rust 1.99.1 / LLVM 23.1.1, x86_64 Linux.
Host hulk: Ryzen 9 5950X, 121 GiB reported RAM, kernel 7.0.0-34-generic,
ext4 on /dev/nvme0n1p2. Input was freshly generated; cache was not flushed,
background load uncontrolled. Cargo configuration and all effective compiler
flags were not exhaustively captured; caller environment and build log are saved.

Published json-volcano 1.0.0, corpus 1, seed 42, records shape, one valid
candidate, depth 1, escape/unicode probabilities 2 percent each. Input size
104,857,600 bytes. Generator reported 0.234767 seconds / 425.955 MiB/s;
its separate GNU time result is retained. FLON read the completed file with
live output, color never, summaries disabled and stdout sent to /dev/null.

| Run | Wall seconds | Input MiB/s | Peak RSS (KiB) | Exit |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1.69 | 59.17 | 3688 | 0 |
| 2 | 1.65 | 60.61 | 3824 | 0 |
| 3 | 1.66 | 60.24 | 3696 | 0 |

Median wall time **1.66 s** (min/max 1.65–1.69); median input throughput
**60.24 MiB/s** (59.17–60.61); median peak RSS **3696 KiB**
(range 3688–3824 KiB). GNU time wall measurements have hundredth-second
precision. No output correctness/hash comparison was performed by this script.
This file-input result is not directly comparable with the earlier pipeline
measurements and establishes no Node/Rust equivalent-work speedup.

Release gate remains open: 1/10 GiB, giant strings, many candidates, deep and
long-lived streams, parser allocation/RSS evidence, memory-only/forced-spill
confirmed output, malformed recovery/cleanup, summary overhead, legacy workload,
and Node/Rust equivalence remain to record.

## Entry template

Copy this section for each version/build:

```text
Date / version / build identifier:
Executable revision and local modifications (verified or unknown):
Generator corpus version, revision and all parameters:
Rust compiler / build flags / Node.js and Node FLON versions if compared:
Host / CPU / RAM / OS / kernel:
Input filesystem / spool filesystem, device and mount options:
Cache conditions / background load:
Raw artifact directory:

Case | mode/options | bytes | repetitions | wall median/min/max | MiB/s | peak RSS | exit status

Correctness: output checks, candidate counts, recovery, cleanup:
Comparison with previous version (same host, parameters and timing method):
Missing cases and reasons:
Known limitations:
```


## 2026-10-23 — completed live and confirmed matrices

Two developer-run campaigns completed with script exit 0; all four
live/confirmed output comparisons passed in each campaign. Raw metadata,
commands, individual timing rows, executable/input hashes and comparisons are
retained; generated fixtures are excluded. Both predate the registry dependency
switch, so they measure the local parser builds captured in their metadata.

| Input / candidates | 250 MiB live median MiB/s | 1 GiB live median MiB/s | 1 GiB confirmed median MiB/s |
| --- | ---: | ---: | ---: |
| Records / 1 | 61.12 | 60.13 | 3.24 |
| Records / 1,024 | 61.43 | 60.74 | 3.37 |
| Giant string / 1 | 109.65 | 108.94 | 100.89 |
| Giant string / 1,024 | 108.23 | 108.70 | 101.59 |

Three measured repetitions per case. [250 MiB summary](performance-results/2026-10-23-250mib-live/summary.txt)
and [1 GiB summary](performance-results/2026-10-23-1gib-live-confirmed/summary.txt)
retain wall-time and RSS ranges. Across these cases process peak RSS remained
roughly 2.5–2.75 MiB. Timing excludes separate output comparisons and generation.
The 250 MiB campaign checks output on separate 1 MiB fixtures; the earlier 1 GiB
campaign checks full-size output. Forced confirmed spill used 64 KiB memory and
unlimited candidate storage. No claim of durable-storage throughput is made.

Confirmed records took median 316.45 s (one candidate) / 303.89 s (1,024),
versus live 17.03 / 16.86 s. Confirmed giant strings took 10.15 / 10.08 s,
versus live 9.40 / 9.42 s. This is a substantial workload-dependent slowdown,
not hash collection time. Code inspection shows the spill file receives each
formatter fragment through unbuffered File::write_all; record formatting makes
many small writes. This is a plausible cause to investigate, not a measured
syscall profile or a verified optimization. Do not hide it by reducing the
correctness fixture size. A buffered spill path would require regression tests
for delayed storage errors, commit flush, discard and cleanup.

Still open: deep/active-key and varied long-lived FLON behavior, any justified
remaining storage/recovery measurements, legacy/Rust equivalent-output checks,
and explicit reasons for omitted larger stress cases. These results do not
complete all release gates or establish comparative speedups.
