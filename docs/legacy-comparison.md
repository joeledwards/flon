# Deferred legacy output comparison

The developer explicitly deferred this exercise on 2026-10-23. It is not a
FLON 1.0 release blocker. No completed Node/Rust comparison or comparative
speedup is claimed; the prepared workspace tool is optional historical tooling.

The original Node implementation is a migration reference. Rust's documented
contract is the intended behavior; matching legacy bugs is not a release goal.
This comparison is a one-time 1.0 exercise, not a permanent compatibility gate
for subsequent Rust releases. No Node/Rust speedup claim follows from this check.

The workspace script `scripts/compare-flon-output` prepares 32 deterministic
seeded fixtures (seed 20261023), four focused matching fixtures, and six
reviewed difference cases. It checks exact stdout bytes, including order, and
retains inputs, expected/actual outputs, hashes, line/byte counts, commands,
metadata and source snapshots under FLON target/compare-output-*.
File input is intentional: the legacy CLI's stdin branch omits its noColor
configuration. Both commands explicitly select plain output.

Reviewed expected differences: raw number spelling, negative zero, integer
precision beyond JavaScript Number, raw value/key escapes and empty nested
containers. Expected outputs are checked independently for each implementation;
other mismatches require review, not normalization or automatic Rust changes.
The cases do not establish universal equivalence or malformed-stream parity.

From the workspace root, with Node dependencies already available:

```bash
python3 scripts/compare-flon-output \
  --toolchain-bin "$PWD/.tools/rust/rustup-home/toolchains/stable-x86_64-unknown-linux-gnu/bin"
```

On 2026-10-23 the developer authorized running this one-time comparison.
The agent attempt built Rust successfully but stopped on the first Node input:
`Cannot find module 'buffered-stream'`, Node v22.22.1. No output comparison
completed. Logs remain in target/compare-output-rumfvqij. No installation,
download, reference-checkout modification or Git mutation was performed.
Three isolated Python tests of the corpus/oracle/report decisions passed;
these are not evidence of executed Node/Rust agreement.
