# flon

FLON (FLat Object Notation) converts JSON into ordered, grep-friendly
assignments without buffering the complete JSON structure.

Live streaming is the default. An optional confirmed mode buffers generated
output for one top-level candidate, spilling to a private temporary file when
needed, and releases it only after the candidate is valid.

This Rust implementation is under development alongside `json-skimmer`. The
implementation includes a streaming formatter library, stdin/file CLI,
recovery policies, bounded confirmed output, and terminal color support.

The terminal implementation uses `anstyle` for semantic styles and
`anstream` for destination-aware output. These are FLON concerns and do not
become dependencies of `json-skimmer`.

## Design goals

- Process unbounded input incrementally.
- Retain only active paths and scalar tokens.
- Preserve object order and JSON number spelling.
- Fail, recover silently, or recover with JavaScript comments when
  `json-skimmer` finds malformed candidates.
- Write to any `std::io::Write` destination.

See the [roadmap](ROADMAP.md) for current implementation status and planned CLI
work.

## Example

Input:

```json
{"name":"flon","items":[1,true]}
```

Plain output:

```javascript
json = {};
json.name = "flon";
json.items = [];
json.items[0] = 1;
json.items[1] = true;
```

## Library use

```rust
let output = flon::format(
    br#"{"name":"flon"}"#.as_slice(),
    Vec::new(),
).expect("valid JSON and writable output");
assert_eq!(output, b"json = {};\njson.name = \"flon\";\n");
```

Use `format_with_config` for key/depth limits, recovery policies, styling,
and confirmed output. `Formatter` also accepts public `json-skimmer` events
directly. Live recovery can leave partial assignments; confirmed mode discards
invalid candidates before they become visible.

## Command-line use

From this repository in the isolated workspace:

```bash
../../scripts/with-rust cargo run -- example.json
../../scripts/with-rust cargo run < example.json
```

URL fetching belongs to tools such as curl, wget, or aqui. Pipe their output
into FLON; there is no built-in URL or TLS configuration:

```bash
curl --fail --silent --show-error https://example.com/data | flon
```

In shell scripts, use `set -o pipefail` to propagate a fetcher's failure even
when FLON successfully processes the bytes received.

Live output with invalid candidates treated as errors is the default. Use
`--help` for the complete configuration surface. Representative modes include:

```bash
# Quiet live recovery, similar to node-flon.
../../scripts/with-rust cargo run -- --profile wraith example.json

# Confirm each candidate, use unlimited file-backed overflow, annotate errors,
# and report recoveries plus a processing summary on stderr.
../../scripts/with-rust cargo run -- --profile infinite example.json

# Bounded confirmed output with explicit binary sizes.
../../scripts/with-rust cargo run -- \
  --output-mode confirmed \
  --confirmed-memory 128MiB \
  --confirmed-limit 10GiB \
  example.json
```

Output uses an 8 KiB buffer by default. Select line flushing for interactive
pipelines or long-lived streams:

```bash
../../scripts/with-rust cargo run -- --buffering=immediate example.json
# Equivalent Node-compatible aliases: --no-buffer or -B
```

`--buffering=buffered|immediate` controls delivery to stdout independently of
`--output-mode=live|confirmed`. Immediate mode flushes each completed output
line; confirmed mode still waits for a valid complete candidate before releasing
any assignments. Both policies flush remaining output on successful EOF.
Partial lines may reach the destination when the bounded buffer fills.

Use `--summary` (or `-v`) to report processing metrics on stderr:

```bash
../../scripts/with-rust cargo run -- --summary example.json
# infinite enables summaries by default; opt out explicitly:
../../scripts/with-rust cargo run -- --profile infinite --no-summary example.json
```

A summary includes elapsed time, input bytes and throughput, output bytes,
candidates started/completed/invalid/recovered, errors, value counts by JSON
type, object-key counts, maximum value depth, and a depth histogram (0–4,
then 5+). Root values have depth zero; keys do not count as values.
Structural statistics include only successfully completed candidates, including
values suppressed by key limits. Invalid or unfinished candidates contribute
no structural counts. Scan mode recognizes object/array candidates; surrounding
text and standalone scalars do not contribute values.

Input bytes include all bytes read, including surrounding text and read-ahead
past an error. Output bytes count successful destination writes below buffering,
including forced ANSI styling and recovery comments, excluding discarded
confirmed output and stderr. They do not guarantee that a final flush succeeded.
Errors count recovered failures plus any terminal processing failure (including
a closed pipe); invalid counts refer specifically to parser-invalid events.
The recovery line reports discarded confirmed-output bytes.

Timing covers input opening through final output flushing using a monotonic
clock. Throughput is input bytes divided by that time. Failed runs and closed
pipes report `failed` and `pipe closed`; stdout remains assignment-only.
Summary colors follow the resolved color policy, with automatic color determined
from **stderr**, independently of stdout. `--diagnostics=quiet` controls recovery
messages independently of summaries. `--show-config` emits no summary.

Nesting is limited to 1,000 open containers by default, counting the root as
depth one. Use `--max-depth N` to select a positive limit. Exceeding it follows
the selected invalid-candidate policy.

Limit raw input bytes per candidate with `--max-candidate-bytes 10MiB`.
The default is `unlimited` (also accepted explicitly); finite limits must be
positive. This works in both live and confirmed modes and does not select an
output mode. Counting includes the root opening/closing delimiters, internal
whitespace, raw escapes, and UTF-8 bytes, excluding surrounding text. Exactly
SIZE bytes are allowed; the first excess byte produces `candidate_limit_exceeded`
before being parsed and follows `--invalid=fail|skip|comment`. Recovery consumes
that byte and scans from the next one, so later nested openers may become new
candidates. At EOF, an unfinished candidate within the limit keeps its ordinary
EOF error. The limit resets for each candidate; it is independent of the
`--confirmed-limit` generated-output bound. Library stream adapters expose it as
`Config::max_candidate_bytes`; direct event consumers configure their parser.

Explicit options override profile defaults. `--show-config` prints the resolved
configuration without processing input.

Color defaults to `auto`: interactive terminal output is styled while pipes
and redirected files remain plain. Override it explicitly when needed:

```bash
../../scripts/with-rust cargo run -- --color=always example.json
../../scripts/with-rust cargo run -- --color=never example.json
```

`--no-color` is an alias for `--color=never`. FLON also recognizes
`FLON_COLOR`, `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and
`TERM=dumb` according to the precedence documented in `docs/color.md`.

## Large JSON fixtures

The companion [json-volcano 1.0](https://crates.io/crates/json-volcano)
library and CLI are published independently. Tests use its crates.io library
with `default-features = false`, without a generator runtime dependency.
It streams exact-size fixtures with bounded memory, including giant candidates,
multiple roots, late errors, and truncated input. See the
[generator guide](docs/json-generator.md) for build commands, options and
separate FLON timing. The generator is not shipped with FLON.

## Project documents

- [Design](docs/design.md) describes the streaming architecture and memory
  model.
- [Compatibility](docs/compatibility.md) defines the relationship with
  `node-flon` and intentional behavior changes.
- [Color](docs/color.md) records the terminal styling and configuration policy.

- [Performance and large-content checks](docs/performance.md) describes the per-version release checklist.
- [Performance results](docs/performance-results.md) retains versioned measurements and baseline comparisons.

- [Test coverage](docs/test-coverage.md) maps behavior to tests and remaining gaps.

## Release plan

Publish initial **0.1.0** releases, json-skimmer first and FLON second. This
makes the current streaming core available while its APIs are still pre-1.0.
The 1.0 stability and validation gates remain tracked in the [roadmap](ROADMAP.md).
See [release instructions](docs/releasing.md) for package checks, publication
order and remaining limitations. Convenience adapters are later 1.x candidates;
no release dates are assigned.
