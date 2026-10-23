# FLON design

## Purpose

FLON converts JSON containers into ordered assignments that are easy to search,
filter, and recombine with line-oriented tools. The Rust implementation is both
a reusable formatter library and a small command-line application.

Its defining constraint is streaming operation: formatting a document must not
require retaining the complete object or array in memory.

## Components

```text
input bytes
    -> json-skimmer
    -> parser events
    -> FLON formatter
        |-- active path state
        |-- plain or styled rendering
        `-- arbitrary Write destination

CLI
    |-- argument and environment policy
    |-- stdin, file, or optional URL source
    |-- output buffering and broken-pipe handling
    `-- timing and diagnostics
```

The formatter library must not depend on process-global stdin, stdout, stderr,
environment variables, terminal detection, or command-line parsing. The binary
owns those concerns and supplies resolved configuration and streams.

## Parser relationship

FLON consumes only the public `json-skimmer` API. It doubles as a real-world
acceptance consumer for that API: `json-skimmer` is not ready for `1.0.0` until
FLON can be implemented without private parser access, duplicate parsing, or
whole-document buffering.

Raw parser fragments are lossless. String fragments retain their JSON escape
sequences and number fragments retain their original spelling. FLON can
therefore write scalar strings and numbers directly after their paths without
decoding and re-encoding them.

## Formatter state

The formatter retains:

- one frame for each active object or array;
- the component by which each nested frame is reached;
- the current raw object key;
- the next index for each active array;
- lexical bookkeeping for the current scalar; and
- output and rendering configuration.

Scalar string and number contents are written as fragments arrive. They are not
accumulated. Object keys must be retained until their values begin because the
key determines the output path. Raw key storage has a configurable per-key
limit of 64 KiB by default. Crossing the limit returns an error by default. An
optional skip policy stops retaining the key, continues parser validation, and
suppresses its complete associated scalar or container value before resuming
with the next object member. Oversized keys are never truncated into paths.

Memory is therefore proportional to nesting depth, active key sizes, output
buffering selected by the caller, and parser limits—not total input size.

## Path rendering

All paths begin with `json`.

- Array elements use zero-based bracket indexes: `json.items[0]`.
- ASCII keys matching `[A-Za-z_$][A-Za-z0-9_$]*` use dot notation:
  `json.name`.
- All other keys use quoted bracket notation: `json["hyphen-ate"]`.

Raw JSON key contents can be used inside bracket notation because the parser
has already validated their escapes. Identifier detection is intentionally
ASCII-only. This is predictable across JavaScript versions and avoids embedding
a Unicode identifier implementation in the formatter.

Array indexes advance when an element begins. Objects and arrays are assigned
at their paths when their opening events arrive, including empty containers.
This preserves encounter order and permits descendants to be emitted
immediately.

## Multiple candidates

The stream is treated as a sequence of updates to one `json` binding. The first
valid candidate writes the root declaration. Later candidates contribute
assignments without reinitializing the root, matching the established
`node-flon` behavior.

This behavior is useful for adjacent JSON and recovered streams but deserves an
explicit future policy if callers need one independent output document per
candidate. A later mode may reset or delimit output between candidates without
changing the default compatibility behavior.

## Invalid candidates and speculative output

`json-skimmer` events are speculative until `CandidateEnd`. The live FLON
formatter writes events immediately, so an invalid candidate may already have
produced assignments or an incomplete scalar line. Those bytes cannot be
retracted from a generic `Write` destination.

Live formatting has three invalid-candidate policies:

- **Fail** is the default and returns an error immediately.
- **Skip** resets candidate state and continues scanning silently.
- **Skip with comment** resets and continues after writing a block comment such
  as `/* flon-error kind=invalid_escape offset=143 candidate_start=97 */`.

Error comments contain only fixed error names and numeric offsets, never input
bytes. They are valid JavaScript when emitted at a statement boundary. Because
live output may already contain an incomplete scalar, annotated live output is
best-effort and is not guaranteed to remain syntactically valid after an
error.

The output buffering modes are:

- **Live:** bounded-memory output with documented partial output on invalid
  candidates.
- **Confirmed:** buffer one candidate and release its output only after
  `CandidateEnd`, subject to a configured size limit.

Live mode is the natural default for unbounded diagnostic streams. Confirmed
mode is appropriate when syntactically complete FLON output is more important
than immediate output. Confirmed storage retains up to 128 MiB in memory by
default, then transfers the current candidate to a private temporary file. The
default per-candidate generated-output limit is 10 GiB; callers can change the
memory threshold and total limit, select a spool directory, or explicitly
choose unlimited storage. These limits reset after every top-level object or
array and never limit cumulative output from the input stream.

At `CandidateEnd`, confirmed output is copied to the destination and its spool
is deleted. On invalidation, it is deleted without becoming visible. Root
declaration state is transactional as well, so the next valid candidate still
produces the declaration when an initial candidate was discarded. A malformed
candidate can therefore be replaced by an error comment while preserving
syntactically valid annotated output.

The formatter reports a structured recovery outcome containing the parse
error, exact number of confirmed-output bytes discarded, and whether storage
had spilled to a file. The library never writes process-global stderr. The CLI
may render these outcomes to stderr according to its diagnostics settings.
Recovery diagnostics remain separate from stdout unless an output annotation
is explicitly requested.

Planned CLI profiles expand into ordinary settings rather than adding library
behavior. `wraith` selects live output, silent invalid-candidate skipping, and
quiet diagnostics. `infinite` selects confirmed output with a larger memory
threshold, unlimited per-candidate spool storage, annotated skipping, and
stderr recovery diagnostics. Explicit CLI options override profile defaults.

## Parser-event diagnostics

FLON will expose a diagnostic mode that renders `json-skimmer` events as
line-delimited JSON. Records include the event type, absolute byte offset or
span, container or string kind where applicable, safely encoded fragment
bytes, structured parse errors, and candidate lifecycle events.

The CLI provides two forms:

- `--output=events` writes event records to stdout instead of FLON
  assignments; and
- `--trace-events` preserves normal FLON output on stdout and writes event
  records to stderr.

Diagnostic rendering is a FLON application concern built from the public
`json-skimmer` event API. It must not make terminal presentation or process
I/O part of the parser. The record format should be defined alongside invalid
candidate and recovery policy so errors and recovery boundaries are visible
without corrupting the assignment stream.

## I/O behavior

The library writes to any `std::io::Write`. A convenience function may read
from any `std::io::Read` using a bounded input buffer.

Output buffering is selected outside the formatter, normally with `BufWriter`.
The CLI must treat a downstream closed pipe as normal successful termination;
this is essential when output is consumed by tools such as `head` or `grep`.

URL fetching, TLS configuration, terminal styling, and timing are application
features. They must not affect the core transformation.

## Error model

Errors fall into three categories:

- input parsing and candidate recovery events from `json-skimmer`;
- inconsistent event sequences, which indicate an API or formatter defect; and
- output or input I/O errors.

Malformed candidates are expected in scanning mode and are not automatically a
fatal library error. Strict input behavior may instead use the parser's
single-container mode.

Both stream adapters preserve the first formatter or I/O error while processing
parser events, including EOF events. After a formatter error, subsequent events
from that parser call are not forwarded to the formatter and cannot trigger
recovery writes or replace the original error. Without an earlier error, EOF
parse errors continue to follow the configured invalid-candidate policy.

The stream adapters use `Config::max_depth`, a positive `NonZeroUsize` with a
default of 1,000, to configure `json-skimmer`. The CLI exposes it as
`--max-depth N`; explicit values override profiles. The root has depth one.
Depth-limit errors follow the selected invalid-candidate policy: confirmed
output is discarded, while live bytes already emitted remain visible. Callers
feeding events directly to `Formatter` must configure their own parser’s
limit; the formatter does not reinterpret parser events as input errors.

The CLI resolves automatic color against stdout before constructing the
formatter. Redirected automatic output generates plain bytes, matching
`--color=never` for confirmed-output thresholds and candidate limits. Forced
color and automatic terminal color retain ANSI bytes in buffer accounting.
