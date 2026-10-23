# Test coverage

Update this matrix when public behavior changes. Library tests live in
`src/lib.rs` and `src/contract_tests.rs`; CLI unit tests live in `src/main.rs`. Shared stream-adapter contract generators live in
`src/stream_test_support.rs` and run against both stream adapters.

| Behavior | Coverage | Remaining gaps |
| --- | --- | --- |
| Plain assignments and paths | Exact fixtures for scalars, nesting, empty containers, duplicate keys, Unicode/escaped keys, number precision | Complete Node fixture port |
| Chunk independence | Exact-output corpus in live and file-backed confirmed modes, every two-chunk split and one-byte chunks | Deterministic random formatter partitions |
| Recovery | Fail/skip/comment policies, path/index reset, transactional root rollback, invalid UTF-8 discard in file-backed confirmed output | Broader malformed-input corpus |
| Depth limits | Default 1000, positive CLI values and profile overrides; shared library/CLI fail/skip/comment recovery checks, memory/file-backed discard, live partial output; chunk divisions | Broader deep-path stress tests |
| Key limits | Exact boundary, overflow error, suppressed scalars/subtrees, validation during skipping | Broader nested chunk combinations |
| Confirmed storage | Invisibility before commit, exact limits, per-candidate reset, spill, discard, cleanup including every truncated prefix of representative 2/3/4-byte UTF-8 characters in keys and values at EOF, in memory and file-backed modes | Cleanup on storage failures; larger stress tests |
| Writers | Two-byte partial writes; byte budgets; BrokenPipe, PermissionDenied, WriteZero propagation in live and confirmed modes | Actual zero-length write responses; flush failures |
| EOF error preservation | Enabled regression; shared library/CLI checks for first-error identity, no writes after failure, all recovery policies, numeric forms, ANSI/plain output, and confirmed limits crossed at EOF in memory/file-backed modes | Broader input I/O failure coverage |
| Color and CLI configuration | Semantic palette, explicit options, profiles, size parsing; auto/always/never across terminal/redirected destinations; exact confirmed limits and no unnecessary spills for redirected auto | Environment precedence; compiled CLI tests |

Run lightweight unit tests from this repository:

```bash
../../scripts/with-rust cargo test --offline --lib --bin flon
```

Compiled CLI integration/e2e tests require explicit authorization.
