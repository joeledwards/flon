# Roadmap

## Priority follow-up from project review

- [x] Validate UTF-8 incrementally in `json-skimmer` so confirmed output never
      releases candidates containing invalid UTF-8.
- [ ] Add configurable parser nesting limits and expose them through FLON so
      deeply nested input cannot grow parser and formatter state without bound.
- [ ] Preserve the first formatter or writer error during EOF processing in
      `format_with_config`; later recovery events must not clear it. Add a
      regression test for a failing writer and truncated numeric input (`[1`).
- [ ] Resolve automatic color against the output destination before formatting
      so stripped ANSI bytes do not inflate confirmed-buffer accounting. Add a
      regression test for redirected output with a candidate size limit.
- [ ] Expand chunk-boundary and invalid-input tests, including UTF-8, nesting
      limits, EOF recovery, and writer failures, before adding more CLI features.
- [ ] Update the README's implementation status to reflect the existing CLI
      and color support, and reconcile parser roadmap items with functionality
      already demonstrated by FLON.

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
