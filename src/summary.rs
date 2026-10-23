use std::cell::Cell;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use json_skimmer::{Event, StringKind};

/// Fixed-size candidate-local counters: fragments and rejected candidates never
/// become part of the completed-value statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Structure {
    pub kinds: [u64; 7],  // object, array, string, number, boolean, null, key
    pub depths: [u64; 6], // 0..4, then 5+
    pub max_depth: u64,
}

impl Structure {
    fn value(&mut self, kind: usize, depth: u64) {
        self.kinds[kind] = self.kinds[kind].saturating_add(1);
        let bucket = depth.min(5) as usize;
        self.depths[bucket] = self.depths[bucket].saturating_add(1);
        self.max_depth = self.max_depth.max(depth);
    }

    fn merge(&mut self, other: &Self) {
        for (total, count) in self.kinds.iter_mut().zip(other.kinds) {
            *total = total.saturating_add(count);
        }
        for (total, count) in self.depths.iter_mut().zip(other.depths) {
            *total = total.saturating_add(count);
        }
        self.max_depth = self.max_depth.max(other.max_depth);
    }
}

#[derive(Debug, Default)]
pub(super) struct Metrics {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub started: u64,
    pub completed: u64,
    pub invalid: u64,
    pub recovered: u64,
    pub discarded_bytes: u64,
    pub structure: Structure,
    pending: Structure,
    depth: u64,
}

impl Metrics {
    pub fn observe(&mut self, event: &Event<'_>) {
        match event {
            Event::CandidateStart { .. } => {
                self.started = self.started.saturating_add(1);
                self.pending = Structure::default();
                self.depth = 0;
            }
            Event::ObjectStart { .. } | Event::ArrayStart { .. } => {
                self.pending.value(
                    usize::from(matches!(event, Event::ArrayStart { .. })),
                    self.depth,
                );
                self.depth = self.depth.saturating_add(1);
            }
            Event::ObjectEnd { .. } | Event::ArrayEnd { .. } => {
                self.depth = self.depth.saturating_sub(1);
            }
            Event::StringStart {
                kind: StringKind::Key,
                ..
            } => {
                self.pending.kinds[6] = self.pending.kinds[6].saturating_add(1);
            }
            Event::StringStart {
                kind: StringKind::Value,
                ..
            } => self.pending.value(2, self.depth),
            Event::NumberStart { .. } => self.pending.value(3, self.depth),
            Event::Boolean { .. } => self.pending.value(4, self.depth),
            Event::Null { .. } => self.pending.value(5, self.depth),
            Event::Invalid(_) => self.invalid = self.invalid.saturating_add(1),
            _ => {}
        }
    }

    pub fn commit(&mut self) {
        self.completed = self.completed.saturating_add(1);
        self.structure.merge(&self.pending);
        self.pending = Structure::default();
    }
}

/// Counts successful destination writes, below CLI buffering and confirmed storage.
pub(super) struct CountWriter<'a, W> {
    pub inner: W,
    pub count: &'a Cell<u64>,
}

impl<W: Write> Write for CountWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        self.count
            .set(self.count.get().saturating_add(count as u64));
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(super) fn run(
    enabled: bool,
    mut diagnostics: impl Write,
    styled: bool,
    work: impl FnOnce(&mut Metrics) -> io::Result<()>,
) -> io::Result<()> {
    let start = enabled.then(Instant::now);
    let mut metrics = Metrics::default();
    let result = work(&mut metrics);
    if let Some(start) = start {
        let summary = write_summary(&mut diagnostics, start.elapsed(), &result, &metrics, styled);
        if result.is_ok() {
            summary?;
        }
    }
    result
}

fn write_summary(
    mut output: impl Write,
    elapsed: Duration,
    result: &io::Result<()>,
    metrics: &Metrics,
    styled: bool,
) -> io::Result<()> {
    let status = match result {
        Ok(()) => "completed",
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => "pipe closed",
        Err(_) => "failed",
    };
    let blue = anstyle::AnsiColor::Blue.on_default();
    let orange = anstyle::Ansi256Color(208).on_default();
    let white = anstyle::AnsiColor::White.on_default();
    let red = anstyle::AnsiColor::Red.on_default();
    macro_rules! key {
        ($value:expr) => {
            SummaryToken {
                value: $value,
                style: blue,
                styled,
            }
        };
    }
    macro_rules! number {
        ($value:expr) => {
            SummaryToken {
                value: $value,
                style: orange,
                styled,
            }
        };
    }
    if styled {
        write!(output, "{white}")?;
    }
    let status = SummaryToken {
        value: status,
        style: if result.is_err() { red } else { white },
        styled,
    };
    writeln!(
        output,
        "{}: {status} in {}s",
        key!("flon"),
        number!(format_args!("{:.3}", elapsed.as_secs_f64()))
    )?;
    let rate = if elapsed.is_zero() {
        0.0
    } else {
        metrics.input_bytes as f64 / elapsed.as_secs_f64()
    };
    writeln!(
        output,
        "  {}: {} bytes ({} bytes/s)",
        key!("input"),
        number!(metrics.input_bytes),
        number!(format_args!("{rate:.0}"))
    )?;
    writeln!(
        output,
        "  {}: {} bytes",
        key!("output"),
        number!(metrics.output_bytes)
    )?;
    writeln!(
        output,
        "  {}: {} {}, {} {}, {} {}, {} {}",
        key!("candidates"),
        number!(metrics.started),
        key!("started"),
        number!(metrics.completed),
        key!("completed"),
        number!(metrics.invalid),
        key!("invalid"),
        number!(metrics.recovered),
        key!("recovered")
    )?;
    writeln!(
        output,
        "  {}: {} ({} {}, {} {})",
        key!("errors"),
        number!(metrics.recovered.saturating_add(u64::from(result.is_err()))),
        number!(metrics.recovered),
        key!("recovered"),
        number!(u8::from(result.is_err())),
        key!("terminal")
    )?;
    let s = &metrics.structure;
    let total = s.kinds[..6]
        .iter()
        .copied()
        .fold(0_u64, u64::saturating_add);
    writeln!(
        output,
        "  {}: {} (completed candidates)",
        key!("values"),
        number!(total)
    )?;
    writeln!(
        output,
        "    {}: {}  {}: {}  {}: {}",
        key!("objects"),
        number!(s.kinds[0]),
        key!("arrays"),
        number!(s.kinds[1]),
        key!("strings"),
        number!(s.kinds[2])
    )?;
    writeln!(
        output,
        "    {}: {}  {}: {}  {}: {}  {}: {}",
        key!("numbers"),
        number!(s.kinds[3]),
        key!("booleans"),
        number!(s.kinds[4]),
        key!("nulls"),
        number!(s.kinds[5]),
        key!("keys"),
        number!(s.kinds[6])
    )?;
    writeln!(
        output,
        "  {}: {} {}",
        key!("depth"),
        key!("max"),
        number!(s.max_depth)
    )?;
    writeln!(
        output,
        "    {}: {}  {}: {}  {}: {}  {}: {}  {}: {}  {}: {}",
        key!("0"),
        number!(s.depths[0]),
        key!("1"),
        number!(s.depths[1]),
        key!("2"),
        number!(s.depths[2]),
        key!("3"),
        number!(s.depths[3]),
        key!("4"),
        number!(s.depths[4]),
        key!("5+"),
        number!(s.depths[5])
    )?;
    writeln!(
        output,
        "  {}: {} confirmed-output bytes discarded",
        key!("recovery"),
        number!(metrics.discarded_bytes)
    )?;
    if styled {
        write!(output, "{white:#}")?;
    }
    output.flush()
}

/// Restore white between semantic tokens so punctuation and units stay neutral.
struct SummaryToken<T> {
    value: T,
    style: anstyle::Style,
    styled: bool,
}

impl<T: std::fmt::Display> std::fmt::Display for SummaryToken<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.styled {
            let white = anstyle::AnsiColor::White.on_default();
            write!(f, "{}{}{white}", self.style, self.value)
        } else {
            self.value.fmt(f)
        }
    }
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
