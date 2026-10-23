# Release roadmap

## Release sequence and scope

Target coordinated **1.0.0 releases** with `json-skimmer`: finish both cores,
publish `json-skimmer` 1.0.0 first, then publish FLON against that dependency.
There are no calendar deadlines or required interim 0.x releases. Publishing,
external checks, and integration/e2e execution remain developer-controlled.

FLON 1.0 provides the agreed Node-compatible core plus the implemented Rust
streaming/recovery improvements. Parser decoding/path convenience adapters are
not dependencies or release gates: FLON already consumes raw public events.
See the [parser release roadmap](../json-skimmer/ROADMAP.md),
[compatibility contract](docs/compatibility.md), and [coverage matrix](docs/test-coverage.md).
Update checklists and coverage notes as work lands; checked items represent
implemented/verified work.

## 1.0.0 — formatter and compatibility

- [x] Formatter library consuming only public `json-skimmer` events
- [x] Ordered plain FLON output to any `Write` destination
- [x] Active object-key paths and array indexes, including empty containers
- [x] Stream raw strings/numbers without accumulating scalar contents
- [x] Preserve JSON escapes, Unicode, and exact number spelling
- [x] Configurable key limits with error/skip behavior
- [x] Configurable nesting limits; default 1,000 containers
- [x] Live output with documented partial-output behavior
- [x] Confirmed output with bounded memory, private spill files, configurable
      per-candidate limits, optional unlimited storage, commit/discard cleanup
- [x] Fail/skip/comment policies and structured recovery outcomes
- [x] Preserve first EOF errors and stop formatter writes after failure
- [x] Port all 20 distinct active Node fixtures and eight Rust extension cases
- [x] Add recovery, Unicode, precision, writer-failure, and chunk-boundary tests
- [x] Review the disabled legacy Node junk fixture: preserve all three strings
      and place the following object at index three; test the corrected output
- [x] Expose parser candidate-span limits and test their
      interaction with live/confirmed output and recovery

## 1.0.0 — CLI and terminal behavior

- [x] Stdin/file input, argument parsing, and help
- [x] Expose recovery policies, key/depth limits, and confirmed-storage settings
- [x] Recovery diagnostics on stderr
- [x] `wraith`/`infinite` profiles and effective-configuration display
- [x] Closed output pipe treated as successful early termination
- [x] Semantic styles with `anstyle` and destination adaptation with `anstream`
- [x] Auto/always/never color, no-color alias, and documented environment policy
- [x] Resolve destination color before formatting and confirmed-buffer accounting
- [x] Add explicit buffered/immediate output selection (distinct from
      live/confirmed candidate validity); define and test flushing behavior
- [x] Add optional processing summaries on stderr (`--summary`/`-v`), enabled
      by default in `infinite` with `--no-summary` override
- [x] Expand summaries with byte/throughput, candidate/error, completed-value type
      and depth metrics; color summaries according to the stderr destination
- [x] Keep URL fetching out of scope; consume curl/wget/aqui output via stdin

## 1.0.0 — acceptance and release gates

- [x] README examples for JSON/plain output, library use, stdin/files, profiles,
      recovery, confirmed output, and color
- [ ] Complete compiled-CLI integration tests for input/output, pipes, errors,
      configuration/environment precedence, and output buffering
- [ ] Validate Node/Rust plain-output equivalence on seeded fixtures before
      claiming equivalent-work speedups (assignment counts/order, bytes/hashes,
      and documented number/escape differences)
- [ ] Benchmark throughput and memory on deep, large-token, and long-lived input,
      including file-backed confirmed output
- [ ] For every released version, complete the [performance checklist](docs/performance.md#per-version-checklist)
      and append version/build metadata, raw timing artifacts, repeated results,
      and missing-case reasons to the [results log](docs/performance-results.md)
- [ ] Verify package-name availability on crates.io
- [ ] Complete licensing files and package metadata
- [ ] Establish and test a minimum supported Rust version
- [ ] Review public formatter/configuration APIs and document stable defaults
- [ ] Run final unit, documentation, formatting, lint, and authorized release checks
- [ ] Update the parser dependency to the published `json-skimmer` 1.0 version
      requirement and verify the publishable package
- [ ] Publish FLON 1.0.0 after the coordinated parser release

## 1.x — additive features guided by actual use

Candidates for minor releases; specific minor versions will be assigned after
scope/design review. None block 1.0.

- [ ] Define a line-delimited JSON parser-event diagnostic format
- [ ] Add `--output=events` for event records on stdout
- [ ] Add `--trace-events` for events/errors on stderr alongside FLON output
- [ ] Add light/dark/custom palettes and role configuration
- [ ] Add other CLI/formatter conveniences as real use identifies a need

## 2.0 and later — changes requiring a breaking-contract review

- [ ] Review any proposal changing established assignment syntax, root/update
      semantics, defaults, library APIs, or CLI behavior before scheduling
- [ ] Keep additive features in 1.x where possible; reserve a major release for
      deliberate incompatible changes with documented migration guidance
