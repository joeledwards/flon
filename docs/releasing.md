# Releasing FLON 1.0.0

The 0.1.0 release is published. Release json-skimmer 1.0.0 first, then FLON
1.0.0. Core formatter/CLI features and the API review are implemented; open
acceptance gates remain recorded in ROADMAP.md.

## Dependency sequence

The developer confirmed json-skimmer 1.0.0 publication. FLON now uses
`json-skimmer = "1.0.0"` from crates.io. Its lockfile records the registry source
and checksum `a17c31d2be00a974a679e9947a02ad81c485b492694f797ce7e174f02bd08ec1`.
Offline locked Cargo metadata confirms registry resolution; no local parser
path or patch override is used. The generator remains a registry-only
library development dependency. No unrelated dependencies were updated.

On 2026-10-23, registry-backed Rust 1.87 checks passed: 45 library and 46 binary
unit tests, documentation tests (zero), documentation build and full package
verification. Stable Clippy lib/bins/tests passed with warnings denied.
All six compiled CLI acceptance tests also passed against the registry
dependency on Rust 1.87 after the developer requested pre-commit verification:
stdin/file/buffering output, profiles, color precedence, invalid/truncated
inputs, confirmed spill/discard/cleanup and broken pipes. This brings the
registry-backed library/binary/CLI total to 97 passing tests. Remaining
performance gates in ROADMAP.md are not cleared by package success.

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
documented evidence reconciliation in docs/performance.md. The developer
explicitly deferred Node/Rust comparison; it is not a publication blocker. Review the package list: license,
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

## Final local validation update (2026-10-23)

After the buffered-spill change and resource-tool additions, agent-run offline
Rust 1.87 checks passed: 48 library, 46 binary and four example unit tests;
documentation tests (zero), documentation generation and package compilation/
verification. Stable formatting and direct clippy-driver lib/bins/examples/tests
with warnings denied passed. Twenty isolated workspace Python tests passed.
Package: 1,144 files, 623.0 KiB / 184.1 KiB compressed. The resource example and
allocator helper are included. Cargo omits the new raw resource artifact directory
because it contains captured Cargo.toml; source-repository evidence remains the
location for that data, not the published crate.

The bounded resource campaign passed all 252 developer-run measurements. Legacy
comparison was explicitly deferred by the developer, with no completed comparison
or Node/Rust speedup claim. Larger omitted stress cases and metric limits are
recorded in the results log. The broader performance checklist is not represented
as fully executed: generator-only/summary-overhead measurements, 100 MiB memory-
only confirmed and GiB recovery campaigns remain unverified here. Review whether
those gaps are accepted for release before closing the performance gate.

The six compiled CLI acceptance tests last passed before buffered spill. A final
rerun is still proposed (not run in this turn), under the separate authorization
rule above. Developer review/Git actions and private publishing checks remain.

## Final release disposition (2026-10-23)

The developer explicitly accepted the documented performance omissions and
authorized the final compiled CLI integration run. All six tests passed on
Rust 1.87 after buffered spill and resource-tool additions. Together with the
98 isolated Rust unit/example tests, zero doc tests, docs/format/lint/package
checks and 20 Python tests already recorded, local validation is complete.
The performance gate is closed with explicit omissions; legacy comparison is
deferred. Earlier pending-check statements above are historical.

Remaining: developer review and commit/push of the working tree and workspace
tooling/evidence, followed by private-context `cargo publish --dry-run --locked`
and `cargo publish --locked`. No Git operation or publication is authorized by
the test/omission approval. Review source-repository evidence links after the
new raw artifacts are committed/pushed; the raw resource directory is omitted
from the crate by Cargo. FLON 1.0.0 publication is not yet confirmed.
