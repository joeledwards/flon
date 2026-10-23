# 1.0 API stability review

The library exports `Formatter<W>`, Read/Write formatting functions,
configuration/policy types, recovery outcomes and default constants. FLON owns
rendering, active paths/keys, confirmed storage and styling; it consumes only
public json-skimmer events. Repository-owned library and CLI code contain no
unsafe code. This is not a transitive-dependency safety audit.

## Contracts to preserve

- Output uses ordered `json` assignments, exact number spelling and raw JSON
  string escapes. Identifier-safe keys use dots; other keys use brackets.
  Repeated candidates update the same root. The compatibility corpus records
  these choices, including intentional corrections to legacy Node behavior.
- Library defaults: plain live output, fail on invalid candidates, 64 KiB raw
  keys with error on overflow, depth 1,000 and unlimited candidate input bytes.
  Confirmed defaults: 128 MiB memory, 10 GiB generated output per candidate,
  default temporary directory. Unlimited storage is explicit.
- Live effects are speculative and cannot be rolled back. Confirmed output
  commits only validated candidates; failure/discard/drop cleans spill storage.
  Commit may partially write before a destination failure: output is not an
  atomic filesystem transaction. Previously committed output remains visible.
- Formatter errors are terminal. Direct event consumers must provide a valid,
  ordered parser event stream and configure parser limits themselves. `into_inner`
  returns the writer; it neither confirms an unfinished candidate nor flushes it.
- Skip/comment applies to candidate errors, not offset exhaustion or I/O/storage
  errors. Offset exhaustion is always fatal. Read adapters propagate input and
  output errors, preserve the first failure, and finalize parsing at EOF.
- CLI defaults: buffered stdout (8 KiB), live/fail, automatic destination color,
  no summary. Immediate output flushes completed lines. Confirmed validity and
  delivery buffering are independent. Summaries/diagnostics use stderr.
  BrokenPipe is successful early CLI termination; library callers receive it.

## Extension review

Public configuration structs support struct literals and enums are exhaustive.
Adding required fields or variants, changing defaults or assignment syntax,
root handling, limits, errors or lifetimes requires a breaking-change review.
New convenience functions or separate configuration types can be additive.
Timing, throughput and fragment callback counts are not stability promises.

Event tracing and palette conveniences are later features. They must preserve
existing default stdout/stderr behavior and configuration precedence. The
coverage matrix and roadmap separately track compiled CLI acceptance, reference
comparisons, performance validation and the tested minimum compiler.
