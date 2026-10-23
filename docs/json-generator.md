# JSON fixture generator

The generator is now **json-volcano**, an independent library/optional-CLI project at
[crates.io](https://crates.io/crates/json-volcano). It is not part of
the FLON package. Standalone corpus-v1 bytes and options are unchanged.
The old `flon-json-gen` executable is no longer built; an old build artifact
may still exist locally, so use `json-volcano` for new runs.

Install the published generator in your normal developer environment:

```bash
cargo install json-volcano --version 1.0.0 --locked --features cli
```

FLON also uses the published library as a development dependency for bounded
fixture tests; it is not a runtime dependency. From the FLON repository,
build FLON:

```bash
../../scripts/with-rust cargo build --offline --release --bin flon
```

Generate a fixture and measure FLON separately:

```bash
json-volcano --bytes 1GiB --seed 42 --stats > large.json
./target/release/flon --color never --summary large.json > /dev/null
```

Or measure the pipeline:

```bash
set -o pipefail
json-volcano --bytes 100MiB --seed 42 --stats |
  ./target/release/flon --no-summary > /dev/null
```

For optional sibling development, build the local generator instead:

```bash
../../scripts/with-rust cargo build --offline --release \
  --manifest-path ../json-volcano/Cargo.toml --features cli
```

Then substitute `../json-volcano/target/release/json-volcano` in the commands
above. This checkout is not required by the published FLON package.

See the [generator guide](https://docs.rs/crate/json-volcano/1.0.0) for all
options and library usage, and the [performance checklist](performance.md)
for repeated versioned measurements. Historical logs retain old command names.
