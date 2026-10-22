# flon

FLON (FLat Object Notation) converts JSON into ordered, grep-friendly
assignments without buffering the complete JSON structure.

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
- Recover and continue when `json-skimmer` finds malformed candidates.
- Write to any `std::io::Write` destination.

See the [roadmap](ROADMAP.md) for current implementation status and planned CLI
work.

## Project documents

- [Design](docs/design.md) describes the streaming architecture and memory
  model.
- [Compatibility](docs/compatibility.md) defines the relationship with
  `node-flon` and intentional behavior changes.
- [Color](docs/color.md) records the terminal styling and configuration policy.
