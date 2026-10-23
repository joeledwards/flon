# Test coverage

Update this matrix when public behavior changes. Library tests live in
`src/lib.rs`, `src/contract_tests.rs`, and `src/compatibility_tests.rs`; CLI unit tests live in `src/main.rs`. Shared stream-adapter contract generators live in
`src/stream_test_support.rs` and run against both stream adapters. CLI delivery
policy tests live in `src/output_tests.rs`; summary metrics tests live in
`src/summary_tests.rs`.

| Behavior | Coverage | Remaining gaps |
| --- | --- | --- |
| Plain assignments and paths | All 20 distinct active Node fixtures plus eight Rust extensions for scalars, nesting, empty containers, duplicate keys, Unicode/escaped keys, number precision; full disabled legacy junk fixture with corrected three-string/fourth-element indexes across chunk divisions in live/confirmed modes | Reference CLI execution |
| Chunk independence | Compatibility corpus in live, memory-backed confirmed, and file-backed confirmed modes; exact-output contract corpus in live/file-backed confirmed modes; every two-chunk split, one-byte chunks, and 16 fixed-seed random partitions per input (1–17 byte chunks, including UTF-8/escape/number boundaries and empty pushes) | Broader malformed-input random partitions |
| Recovery | Fatal offset exhaustion under all recovery policies; fail/skip/comment policies, path/index reset, transactional root rollback, invalid UTF-8 discard in file-backed confirmed output | Broader malformed-input corpus |
| Depth limits | Default 1000, positive CLI values and profile overrides; shared library/CLI fail/skip/comment recovery checks, memory/file-backed discard, live partial output; chunk divisions | Broader deep-path stress tests |
| Candidate input limits | Shared public Read-adapter corpus for library/CLI; exact boundaries, Unicode/escapes/numbers, every split and one-byte chunks, fail/skip/comment, live partial output, confirmed memory/spill discard and cleanup, independent output limits, profile/default/size validation, summary rollback | Compiled CLI checks |
| Key limits | Exact boundary, overflow error, suppressed scalars/subtrees, validation during skipping | Broader nested chunk combinations |
| Streaming fixture generator | Published json-volcano 1.0 dev-dependency: exact string assignments with Unicode/escapes, valid/invalid/mixed/truncated commit/discard, whole/every split/one-byte input, memory/spill and cleanup in `published_volcano_fixtures_commit_valid_candidates_and_discard_failures`. Generator owned and unit-tested by the companion [json-volcano project](../../json-volcano/README.md); FLON usage documented in [generator guide](json-generator.md) | Node/Rust plain-output equivalence; actual GiB-scale runs and measured RSS |
| Large randomized content | Fixed-seed valid/late-invalid/mixed candidates with independent output digest oracle; memory/immediate-spill/transition modes; one-byte/4093/8192-byte reads; large truncation, output-limit failure and formatter-drop cleanup; Linux internal spool ENOSPC via `/dev/full`; ignored tunable release workload with versioned JSONL metrics | Real quota-limited filesystem ENOSPC across full adapters, creation/copy failures, descriptor/crash cleanup, RSS/disk counters; see [performance guide](performance.md) |
| Confirmed storage | Invisibility before commit, exact limits, per-candidate reset, spill, discard, cleanup including every truncated prefix of representative 2/3/4-byte UTF-8 characters in keys and values at EOF, in memory and file-backed modes; missing-directory spill-creation errors release no output (immediate spill and memory-to-file transition); memory/file-backed commit failures preserve exact partial destination writes and clean spool storage across recovery policies | Injected spool creation/copy/read/seek failures; full-adapter ENOSPC (internal file write covered); see large-content row |
| Summaries | Exact text/JSON/FLON fixtures; format enable/conflict/default/profile and stdout separation; human byte/count/time units, exact large integers, zero/long durations, dynamically sized ASCII borders and styled equivalence; all-format partial writes/flush errors and processing-error precedence; exact plain/styled output, semantic label/number/separator/status colors, zero/long durations, policy/alias/override, separate stdout/stderr, disabled silence, writer failure precedence/partial writes; completed-value types/keys/depths across every split and one-byte chunks, Unicode/escapes/numbers, recovery/invalid UTF-8/truncation rollback, histogram overflow, suppressed values, depth/key/storage limits, partial destination writes, broken pipes, read/final-flush errors, ANSI byte accounting and memory/file-backed discard | Broader compiled summary/environment matrix; per-format compiled CLI matrix and spill totals remain open |
| CLI output buffering | Buffered default, immediate/no-buffer/-B selection and conflicts, profile independence, line visibility before next input read, confirmed withholding, multi-line commits, final partial lines, large lines, partial writes and flush failures | Interactive/TTY delivery timing; compiled tests cover finite-input policy selections |
| Writers | Two-byte partial writes; byte budgets; BrokenPipe, PermissionDenied, WriteZero propagation in live and confirmed modes; actual `Ok(0)` responses before/after partial writes and candidate boundaries in live, memory-backed confirmed, and file-backed confirmed modes across fail/skip/comment policies; exact output prefixes, no further writes/reads after failure, spool cleanup | Broader writer-failure corpus |
| EOF error preservation | Enabled regression; shared library/CLI checks for first-error identity, no writes after failure, all recovery policies, numeric forms, ANSI/plain output, and confirmed limits crossed at EOF in memory/file-backed modes | Broader input I/O failure coverage |
| Color and CLI configuration | Semantic palette, explicit options, profiles, size parsing; auto/always/never across terminal/redirected destinations; exact confirmed limits and no unnecessary spills for redirected auto; table-driven CLI/environment precedence (all explicit choices, FLON_COLOR, NO_COLOR presence, force-color empty/zero values, CLICOLOR/TERM, invalid and non-Unicode values) with isolated lookups and terminal/redirected decisions | Broader compiled environment matrix; unit precedence matrix is more extensive |

Run lightweight unit tests from this repository:

```bash
../../scripts/with-rust cargo test --offline --lib --bins
```

Compiled CLI integration/e2e tests require explicit authorization.

## Compiled CLI acceptance suite

The developer ran `tests/cli.rs` successfully on Rust 1.87.0 on 2026-10-23:
all six compiled CLI acceptance tests passed, alongside 45 library and 41
binary unit tests. The manifest now declares Rust 1.87.
It covers stdin/dash/file input, buffering selections, exact Unicode/object
output, confirmed spill/recovery/cleanup, failure exit statuses, environment
color precedence, profile overrides, configuration-only exit and Unix closed
output. Unit tests still provide the fine-grained delivery/writer/error checks.
Interactive timing/TTY behavior remains a gap. Run only with explicit local
integration-test authorization:

```bash
../../scripts/with-rust cargo test --offline --locked --test cli
```

Workspace measurement tooling: `scripts/test_measure_flon_matrix.py` checks
native hash/count output and failures, timing summaries and selection of an
independent smaller correctness fixture without changing mode/storage options.
These isolated tests do not execute FLON benchmarks. Small-fixture digest
agreement does not prove large timed-output equivalence; selected large spill
and storage-boundary evidence remains required.
