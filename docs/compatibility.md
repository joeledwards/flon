# node-flon compatibility

## Compatibility target

The Rust implementation preserves the useful behavior and output vocabulary of
the reference `node-flon` project. It is not intended to preserve bugs,
dependency quirks, or reliance on Clarinet internals.

Compatibility is evaluated with fixtures and semantic output, not internal
implementation similarity.

## Behavior to preserve

- The root binding is named `json`.
- Root objects and arrays are declared as `json = {};` and `json = [];`.
- Nested containers are declared before their descendants.
- Values remain in input encounter order.
- Identifier-safe keys use dot notation.
- Other keys use quoted bracket notation.
- Arrays use zero-based indexes.
- Strings, numbers, booleans, and null produce one assignment each.
- Empty objects and arrays still produce declarations.
- Multiple consecutive candidates contribute to the same root binding.
- Processing is incremental and does not build the complete JSON value.
- Plain output contains no terminal escape sequences.

## Intentional improvements

### Parser recovery

`node-flon` resets private Clarinet state after an error, and its junk-data test
is disabled. Rust FLON uses explicit `json-skimmer` candidate and invalid events
with documented recovery semantics. Junk between valid objects or arrays is a
supported and tested input form.

### Escaping

Keys and strings must remain valid and unambiguous for all JSON escapes,
backslashes, control characters, and Unicode input. The Rust implementation
preserves validated raw string fragments rather than applying the Node
implementation's quote-only replacement.

### Numbers

Number spelling and precision are preserved. Values are not converted through
JavaScript `Number` or Rust floating-point types. Large integers, exponents,
and insignificant-looking fractional zeros remain as written.

### Resource control

Nesting depth, active key size, parser spans, and any confirmed-output buffer
must have explicit configurable limits. Limit failures are reported rather than
allowing accidental unbounded allocation.

### Stable APIs

The formatter depends only on public `json-skimmer` interfaces. It does not
replace parser internals after an error.

## Fixture matrix

The compatibility suite must include:

- empty and populated root objects;
- empty and populated root arrays;
- every scalar type;
- nested objects inside arrays and arrays inside objects;
- nested arrays inside arrays;
- identifier, hyphenated, empty, quoted, escaped, and Unicode keys;
- escaped and Unicode string values;
- exact large, fractional, and exponent number spellings;
- multiple adjacent or newline-separated candidates;
- junk between valid candidates;
- malformed and truncated candidates followed by valid candidates;
- every meaningful division of input into chunks; and
- output writers that accept partial writes or report broken pipes.

## CLI compatibility

The initial Rust CLI should provide equivalents for the high-value Node flags:

- file input;
- color disablement and explicit color choice;
- buffered versus immediate output; and
- optional elapsed-time summary.

URL input and disabling TLS certificate validation require a deliberate safety
and dependency decision. If insecure TLS is retained, it must be explicit,
local to the selected request, and accompanied by a visible warning.

The Rust CLI additionally provides parser-event diagnostics for debugging
malformed and recovered streams. This is an intentional extension rather than
a `node-flon` compatibility requirement. Event-only output may use stdout;
event tracing alongside FLON assignments uses stderr.

Exact short-option compatibility is desirable where it does not conflict with
clear modern CLI conventions. Environment variable names may evolve, with
documented compatibility aliases where valuable.

## Acceptance criterion

Rust FLON is compatible when the reference fixtures and the expanded matrix
produce the agreed plain output while using memory proportional to active depth,
active keys, configured parser limits, and selected output policy.
