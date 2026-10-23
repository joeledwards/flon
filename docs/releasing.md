# Releasing FLON 1.0.0

The 0.1.0 release is published. Release json-skimmer 1.0.0 first, then FLON
1.0.0. Core formatter/CLI features and the API review are implemented; open
acceptance gates remain recorded in ROADMAP.md.

## Dependency sequence

During coordinated preparation, FLON uses a version+path dependency on the
local json-skimmer 1.0.0. This verifies parser changes before publication.
After parser publication, remove the path, leaving `json-skimmer = "1.0.0"`,
fetch that registry package and refresh Cargo.lock. Verify parser and generator
entries have registry sources/checksums and no patch override is active.
Final package verification must use that actual published artifact.

## Local checks

From this repository, using the workspace-local toolchain:

```bash
../../scripts/with-rust cargo fmt --check
../../scripts/with-rust cargo test --offline --locked --lib --bins
../../scripts/with-rust cargo test --offline --locked --doc
../../scripts/with-rust cargo clippy --offline --locked --lib --bins --tests -- -D warnings
../../scripts/with-rust cargo doc --offline --locked --no-deps
../../scripts/with-rust cargo package --offline --locked --allow-dirty --list
../../scripts/with-rust cargo package --offline --locked --allow-dirty
```

The compiled CLI suite is separate and needs explicit integration-test
authorization under the workspace rules:

```bash
../../scripts/with-rust cargo test --offline --locked --test cli
```

The declared minimum compiler is Rust 1.87. The developer passed all 92
library/binary/compiled-CLI tests on Rust 1.87.0. Complete the performance and
reference-output checks in docs/performance.md. Review the package list: license,
docs, source/tests and example included; root performance.jsonl and generated
large inputs excluded. `--allow-dirty` permits review without Git mutations.
The developer reviews and commits the final release before publishing.

## Publishing (developer-run)

Use a private publishing credential context, not the workspace-local Cargo home:

```bash
cargo publish --dry-run --locked
cargo publish --locked
```

## Scope and stable contract

See [API stability](api-stability.md) for defaults, streaming/confirmed behavior,
error propagation and extension policy. Event diagnostics and palette
conveniences remain later features. URL fetching stays with external tools.
Open validation gates must not be marked complete based only on unit results.
