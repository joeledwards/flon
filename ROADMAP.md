# Roadmap

## Streaming formatter

- [x] Library-first formatter accepting `json-skimmer` events
- [x] Plain FLON output to any `std::io::Write`
- [x] Streaming scalar strings and numbers without accumulating their contents
- [x] Active object-key and array-index path tracking
- [x] Initial compatibility fixtures based on `node-flon`
- [x] Compile, format, and run the initial test suite
- [x] Add configurable active-key length limits
- [x] Define partial-output behavior for invalid speculative candidates
- [x] Add bounded or explicitly unlimited confirmed-output mode
- [x] Report structured invalid-candidate recovery outcomes
- [ ] Define a line-delimited JSON parser-event diagnostic format
- [ ] Port the complete `node-flon` fixture set
- [ ] Add recovery, Unicode, escaping, precision, and chunk-boundary fixtures

## Command-line application

- [x] Add stdin and file input
- [ ] Add buffered and immediate output modes
- [x] Add CLI configuration and help
- [x] Add `--invalid=fail|skip|comment`
- [x] Expose key and confirmed-buffer limits through the CLI
- [x] Report invalid-candidate recovery diagnostics on stderr
- [ ] Add `--output=events` for parser events instead of FLON assignments
- [ ] Add `--trace-events` for parser events and errors on stderr
- [x] Add `wraith` and `infinite` CLI configuration profiles
- [x] Add effective-configuration display for profiles and overrides
- [x] Treat a closed output pipe as successful early termination
- [ ] Add elapsed-time summaries
- [ ] Decide whether URL input belongs in the initial Rust release
- [ ] Add URL input and explicit TLS policy if retained

## Terminal presentation

- [x] Use `anstyle` for the semantic FLON palette
- [x] Use `anstream` for styled output and terminal adaptation
- [x] Support `--color=auto|always|never`
- [x] Honor `NO_COLOR` and document environment-variable precedence
- [x] Support `FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` compatibility
- [ ] Add light, dark, and custom FLON palette configuration

## Release readiness

- [ ] Verify whether the `flon` package name is available on crates.io
- [ ] Add README examples for library use, stdin, files, profiles, recovery,
      confirmed output, and color before publishing
- [ ] Show representative JSON input and plain FLON output in the README
- [ ] Add licensing and complete package metadata
- [ ] Establish a minimum supported Rust version
- [ ] Add integration tests for the compiled CLI
- [ ] Benchmark throughput and memory use on deeply nested and long-lived input
- [ ] Publish only after the required `json-skimmer` API is available on crates.io
