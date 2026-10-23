# Releasing FLON 0.1.0

Publish json-skimmer 0.1.0 first, then this initial pre-1.0 formatter/CLI. Core
formatter and CLI features are implemented; unchecked 1.0 acceptance gates
remain work for later releases. json-volcano 1.0.0 is already a crates.io
library-only development dependency.

## Dependency sequence

json-skimmer 0.1.0 is published. FLON now uses `json-skimmer = "0.1.0"`
from crates.io, without a sibling path or patch. Verify parser and generator
lock entries contain registry sources and checksums, and run the checks below
against those registry artifacts before publication.

## Local checks

From this repository, using the workspace-local toolchain:

```bash
../../scripts/with-rust cargo fmt --check
../../scripts/with-rust cargo test --offline --locked --lib --bins
../../scripts/with-rust cargo test --offline --locked --doc
../../scripts/with-rust cargo clippy --offline --locked --lib --bins -- -D warnings
../../scripts/with-rust cargo doc --offline --locked --no-deps
../../scripts/with-rust cargo package --offline --locked --allow-dirty --list
../../scripts/with-rust cargo package --offline --locked --allow-dirty
```

Package compilation needs the published parser dependency available in the
registry cache. Review package contents and ensure root performance.jsonl and
large generated input files are excluded. `--allow-dirty` allows reviewing the
prepared files without an agent changing Git state. The developer reviews and
commits the release before publishing.

The workspace's `scripts/check-flon-pipeline` is a developer-run compiled CLI
smoke check, separate from unit tests. It builds the local generator binary,
uses the published generator library dependency, and checks confirmed-output
recovery, spill and cleanup. It does not replace a complete CLI acceptance suite
or performance validation. Follow docs/performance.md for optional heavy checks.

## Publishing (developer-run)

Use a private publishing credential context, not the workspace-local Cargo home:

```bash
cargo publish --dry-run --locked
cargo publish --locked
```

## Pre-1.0 limitations

No MSRV is established. Fuller compiled-CLI acceptance, independent Node/Rust
output comparisons, repeated throughput/RSS/storage measurements and stable API
review remain open. URL fetching is intentionally delegated to external tools.
See ROADMAP.md and docs/test-coverage.md; do not claim all 1.0 gates are complete.
