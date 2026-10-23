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

Live output with invalid candidates treated as errors is the default. Use
`--help` for the complete configuration surface. Representative modes include:

```bash
# Quiet live recovery, similar to node-flon.
../../scripts/with-rust cargo run -- --profile wraith example.json

# Confirm each candidate, use unlimited file-backed overflow, annotate errors,
# and report recoveries on stderr.
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

Nesting is limited to 1,000 open containers by default, counting the root as
depth one. Use `--max-depth N` to select a positive limit. Exceeding it follows
the selected invalid-candidate policy.

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

## Project documents

- [Design](docs/design.md) describes the streaming architecture and memory
  model.
- [Compatibility](docs/compatibility.md) defines the relationship with
  `node-flon` and intentional behavior changes.
- [Color](docs/color.md) records the terminal styling and configuration policy.

- [Test coverage](docs/test-coverage.md) maps behavior to tests and remaining gaps.

## Release plan

The target is a coordinated 1.0.0 release with `json-skimmer`.
The parser publishes first, then FLON uses that published dependency. Core
features and release gates are tracked in [the release roadmap](ROADMAP.md);
convenience features are candidates for later 1.x releases, with breaking
changes reviewed for 2.0. No release dates are assigned.
