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
