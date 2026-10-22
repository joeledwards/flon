# flon

FLON (FLat Object Notation) converts JSON into ordered, grep-friendly
assignments without buffering the complete JSON structure.

Live streaming is the default. An optional confirmed mode buffers generated
output for one top-level candidate, spilling to a private temporary file when
needed, and releases it only after the candidate is valid.

This Rust implementation is under development alongside `json-skimmer`. The
initial implementation is a plain-text formatter library; CLI and terminal
color support will follow after the streaming contract is stable.

The planned terminal implementation uses `anstyle` for semantic styles and
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
