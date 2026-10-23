# Processing summary formats

`--summary-format text|json|flon` enables processing summaries on stderr.
The default format is text; `--summary`/`-v` and the infinite profile select
text unless a format is explicit. `--no-summary` conflicts with format selection
and with `--summary`. `--show-config` prints configuration and exits without a
summary. Assignment output on stdout is independent of summary format.

Text is a human presentation, one metric per line, with blank lines around an
`=` header and `-` footer sized to the widest visible line. Colors do not affect
the width. Bytes use powers of 1024 (B through EiB); count suffixes use powers
of 1000 (k through E). Human values are approximate, rounded to two decimal
places; literal bytes, counts and elapsed nanoseconds remain exact. Small
counts appear once. Durations use ns/µs/ms/s/min/h as appropriate. This is not a
fixed-column terminal table; long exact values can exceed the terminal width.

JSON is a single newline-terminated object. FLON declares `summary = {};`
then writes `summary.<field> = <literal>;` in the same order as JSON. Neither
machine format contains ANSI codes, decorative separators or human suffixes.
Fields are always present, including zeros:

| Field | Type / unit | Meaning |
| --- | --- | --- |
| `status` | string | `completed`, `failed`, or `pipe closed` |
| `elapsed_ns` | nonnegative integer, ns | Monotonic time from input opening through final output flush |
| `input_bytes` | nonnegative integer, bytes | All bytes read, including read-ahead and surrounding junk |
| `output_bytes` | nonnegative integer, bytes | Successful destination writes, including ANSI/comments, excluding discarded output and stderr |
| `input_bytes_per_second` | nonnegative number, bytes/s | Input bytes / elapsed time, or zero when elapsed is zero |
| `candidates_started`, `candidates_completed`, `candidates_invalid`, `candidates_recovered` | nonnegative integer counts | Candidate lifecycle totals |
| `errors`, `errors_recovered`, `errors_terminal` | nonnegative integer counts | Recovered plus terminal failures; terminal is zero or one, including closed pipes |
| `values` | nonnegative integer count | Sum of completed object/array/string/number/boolean/null values |
| `objects`, `arrays`, `strings`, `numbers`, `booleans`, `nulls`, `keys` | nonnegative integer counts | Completed-candidate type/key totals |
| `max_depth` | nonnegative integer depth | Maximum completed value depth; root is zero |
| `depth_0`, `depth_1`, `depth_2`, `depth_3`, `depth_4`, `depth_5_plus` | nonnegative integer counts | Completed-value depth histogram |
| `discarded_bytes` | nonnegative integer, bytes | Confirmed output discarded during recovery |

Integer counters saturate at u64::MAX; elapsed nanoseconds can exceed u64.
Consumers must preserve integer precision (for example, do not decode all JSON
numbers as IEEE-754 doubles). Throughput uses f64 arithmetic; its literal is the
computed value without additional display rounding, not an exact rational.
It is finite, including empty/zero-duration runs. Structural totals include
key-suppressed values in completed candidates; invalid/truncated candidates
contribute none. Byte counts do not guarantee that a final flush succeeded.

Field meanings/types are part of the 1.0 machine contract; additive fields may
be introduced in minor releases. Consumers should ignore unknown fields and
must not depend on field order or text layout. Timing and throughput vary by run.

Diagnostics and final CLI errors share stderr. `--diagnostics quiet` suppresses
recovery messages but does not suppress final errors; a failed run can append
an error line after the structured summary. Summary writes/flushes can fail;
the original processing error takes precedence over summary failures. Closed
pipes remain successful early CLI termination even though the summary reports
`pipe closed` and one terminal error.
