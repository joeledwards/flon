# Test coverage

Update this matrix when public behavior changes. Library tests live in
`src/lib.rs`, `src/contract_tests.rs`, and `src/compatibility_tests.rs`; CLI unit tests live in `src/main.rs`. Shared stream-adapter contract generators live in
`src/stream_test_support.rs` and run against both stream adapters. CLI delivery
policy tests live in `src/output_tests.rs`; summary metrics tests live in
`src/summary_tests.rs`.

| Behavior | Coverage | Remaining gaps |
| --- | --- | --- |
| Plain assignments and paths | All 20 distinct active Node fixtures plus eight Rust extensions for scalars, nesting, empty containers, duplicate keys, Unicode/escaped keys, number precision; full disabled legacy junk fixture with corrected three-string/fourth-element indexes across chunk divisions in live/confirmed modes | Reference CLI execution |
| Chunk independence | Exact-output corpus in live and file-backed confirmed modes, every two-chunk split and one-byte chunks | Deterministic random formatter partitions |
| Recovery | Fail/skip/comment policies, path/index reset, transactional root rollback, invalid UTF-8 discard in file-backed confirmed output | Broader malformed-input corpus |
| Depth limits | Default 1000, positive CLI values and profile overrides; shared library/CLI fail/skip/comment recovery checks, memory/file-backed discard, live partial output; chunk divisions | Broader deep-path stress tests |
| Candidate input limits | Shared public Read-adapter corpus for library/CLI; exact boundaries, Unicode/escapes/numbers, every split and one-byte chunks, fail/skip/comment, live partial output, confirmed memory/spill discard and cleanup, independent output limits, profile/default/size validation, summary rollback | Compiled CLI checks |
| Key limits | Exact boundary, overflow error, suppressed scalars/subtrees, validation during skipping | Broader nested chunk combinations |
| Confirmed storage | Invisibility before commit, exact limits, per-candidate reset, spill, discard, cleanup including every truncated prefix of representative 2/3/4-byte UTF-8 characters in keys and values at EOF, in memory and file-backed modes; missing-directory spill-creation errors release no output (immediate spill and memory-to-file transition); memory/file-backed commit failures preserve exact partial destination writes and clean spool storage across recovery policies | Injected disk-write/read/seek failures; larger stress tests |
| Summaries | Exact plain/styled output, semantic label/number/separator/status colors, zero/long durations, policy/alias/override, separate stdout/stderr, disabled silence, writer failure precedence/partial writes; completed-value types/keys/depths across every split and one-byte chunks, Unicode/escapes/numbers, recovery/invalid UTF-8/truncation rollback, histogram overflow, suppressed values, depth/key/storage limits, partial destination writes, broken pipes, read/final-flush errors, ANSI byte accounting and memory/file-backed discard | Compiled CLI stderr/environment checks; size extremes and spill totals not implemented |
| CLI output buffering | Buffered default, immediate/no-buffer/-B selection and conflicts, profile independence, line visibility before next input read, confirmed withholding, multi-line commits, final partial lines, large lines, partial writes and flush failures | Compiled CLI/pipe checks |
| Writers | Two-byte partial writes; byte budgets; BrokenPipe, PermissionDenied, WriteZero propagation in live and confirmed modes | Actual zero-length write responses |
| EOF error preservation | Enabled regression; shared library/CLI checks for first-error identity, no writes after failure, all recovery policies, numeric forms, ANSI/plain output, and confirmed limits crossed at EOF in memory/file-backed modes | Broader input I/O failure coverage |
| Color and CLI configuration | Semantic palette, explicit options, profiles, size parsing; auto/always/never across terminal/redirected destinations; exact confirmed limits and no unnecessary spills for redirected auto; table-driven CLI/environment precedence (all explicit choices, FLON_COLOR, NO_COLOR presence, force-color empty/zero values, CLICOLOR/TERM, invalid and non-Unicode values) with isolated lookups and terminal/redirected decisions | Compiled CLI tests |

Run lightweight unit tests from this repository:

```bash
../../scripts/with-rust cargo test --offline --lib --bin flon
```

Compiled CLI integration/e2e tests require explicit authorization.
