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

/// Machine formats always use unstyled literal values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, clap::ValueEnum)]
pub(super) enum SummaryFormat {
    #[default]
    Text,
    Json,
    Flon,
}

pub(super) fn run(
    enabled: bool,
    mut diagnostics: impl Write,
    styled: bool,
    format: SummaryFormat,
    work: impl FnOnce(&mut Metrics) -> io::Result<()>,
) -> io::Result<()> {
    let start = enabled.then(Instant::now);
    let mut metrics = Metrics::default();
    let result = work(&mut metrics);
    if let Some(start) = start {
        let summary = write_formatted_summary(
            &mut diagnostics,
            start.elapsed(),
            &result,
            &metrics,
            styled,
            format,
        );
        if result.is_ok() {
            summary?;
        }
    }
    result
}

#[cfg(test)]
fn write_summary(
    output: impl Write,
    elapsed: Duration,
    result: &io::Result<()>,
    metrics: &Metrics,
    styled: bool,
) -> io::Result<()> {
    write_formatted_summary(
        output,
        elapsed,
        result,
        metrics,
        styled,
        SummaryFormat::Text,
    )
}

// Human quantities are approximate; literal integer fields never pass through f64.
fn quantity(value: f64, units: &[&str], base: f64) -> String {
    let mut scaled = value;
    let mut unit = 0;
    while scaled >= base && unit + 1 < units.len() {
        scaled /= base;
        unit += 1;
    }
    format!("{scaled:.2} {}", units[unit]).trim_end().to_owned()
}

struct Field {
    name: &'static str,
    label: &'static str,
    literal: String,
    readable: String,
}

fn write_formatted_summary(
    mut output: impl Write,
    elapsed: Duration,
    result: &io::Result<()>,
    metrics: &Metrics,
    styled: bool,
    format: SummaryFormat,
) -> io::Result<()> {
    let status = match result {
        Ok(()) => "completed",
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => "pipe closed",
        Err(_) => "failed",
    };
    let mut fields = Vec::new();
    let mut add = |name, label, literal: String, readable| {
        fields.push(Field {
            name,
            label,
            literal,
            readable,
        });
    };
    let seconds = elapsed.as_secs_f64();
    let duration = if seconds < 1.0 {
        quantity(seconds * 1e9, &["ns", "µs", "ms", "s"], 1000.0)
    } else if seconds < 60.0 {
        format!("{seconds:.2} s")
    } else if seconds < 3600.0 {
        format!("{:.2} min", seconds / 60.0)
    } else {
        format!("{:.2} h", seconds / 3600.0)
    };
    add(
        "elapsed_ns",
        "elapsed",
        elapsed.as_nanos().to_string(),
        format!("{duration} ({} ns)", elapsed.as_nanos()),
    );
    let bytes = |value: u64| {
        format!(
            "{} ({value} bytes)",
            quantity(
                value as f64,
                &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"],
                1024.0
            )
        )
    };
    add(
        "input_bytes",
        "input",
        metrics.input_bytes.to_string(),
        bytes(metrics.input_bytes),
    );
    add(
        "output_bytes",
        "output",
        metrics.output_bytes.to_string(),
        bytes(metrics.output_bytes),
    );
    let rate = if elapsed.is_zero() {
        0.0
    } else {
        metrics.input_bytes as f64 / seconds
    };
    add(
        "input_bytes_per_second",
        "input throughput",
        rate.to_string(),
        format!(
            "{}/s ({rate} bytes/s)",
            quantity(
                rate,
                &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"],
                1024.0
            )
        ),
    );
    let s = &metrics.structure;
    let total = s.kinds[..6]
        .iter()
        .copied()
        .fold(0_u64, u64::saturating_add);
    for (name, label, value) in [
        ("candidates_started", "candidates started", metrics.started),
        (
            "candidates_completed",
            "candidates completed",
            metrics.completed,
        ),
        ("candidates_invalid", "candidates invalid", metrics.invalid),
        (
            "candidates_recovered",
            "candidates recovered",
            metrics.recovered,
        ),
        (
            "errors",
            "errors",
            metrics.recovered.saturating_add(u64::from(result.is_err())),
        ),
        ("errors_recovered", "errors recovered", metrics.recovered),
        (
            "errors_terminal",
            "errors terminal",
            u64::from(result.is_err()),
        ),
        ("values", "values (completed candidates)", total),
        ("objects", "objects", s.kinds[0]),
        ("arrays", "arrays", s.kinds[1]),
        ("strings", "strings", s.kinds[2]),
        ("numbers", "numbers", s.kinds[3]),
        ("booleans", "booleans", s.kinds[4]),
        ("nulls", "nulls", s.kinds[5]),
        ("keys", "keys", s.kinds[6]),
        ("max_depth", "maximum value depth", s.max_depth),
        ("depth_0", "values at depth 0", s.depths[0]),
        ("depth_1", "values at depth 1", s.depths[1]),
        ("depth_2", "values at depth 2", s.depths[2]),
        ("depth_3", "values at depth 3", s.depths[3]),
        ("depth_4", "values at depth 4", s.depths[4]),
        ("depth_5_plus", "values at depth 5+", s.depths[5]),
    ] {
        let readable = if value < 1000 {
            value.to_string()
        } else {
            format!(
                "{} ({value})",
                quantity(value as f64, &["", "k", "M", "G", "T", "P", "E"], 1000.0)
            )
        };
        add(name, label, value.to_string(), readable);
    }
    add(
        "discarded_bytes",
        "confirmed output discarded",
        metrics.discarded_bytes.to_string(),
        bytes(metrics.discarded_bytes),
    );

    match format {
        SummaryFormat::Json => {
            // Status is one of three fixed ASCII strings; keys are fixed identifiers.
            write!(output, "{{\"status\":\"{status}\"")?;
            for field in &fields {
                write!(output, ",\"{}\":{}", field.name, field.literal)?;
            }
            writeln!(output, "}}")?;
        }
        SummaryFormat::Flon => {
            writeln!(output, "summary = {{}};\nsummary.status = \"{status}\";")?;
            for field in &fields {
                writeln!(output, "summary.{} = {};", field.name, field.literal)?;
            }
        }
        SummaryFormat::Text => {
            let title = "FLON processing summary";
            // All labels are ASCII; µ is a single visible column. No ANSI bytes
            // participate in this calculation.
            let width = fields
                .iter()
                .map(|field| field.label.chars().count() + 2 + field.readable.chars().count())
                .chain([title.len(), "status: ".len() + status.len()])
                .max()
                .unwrap();
            let white = anstyle::AnsiColor::White.on_default();
            if styled {
                write!(output, "{white}")?;
            }
            writeln!(output, "\n{}\n{title}", "=".repeat(width))?;
            let label_style = anstyle::AnsiColor::Blue.on_default();
            let number_style = anstyle::Ansi256Color(208).on_default();
            let status_style = if result.is_err() {
                anstyle::AnsiColor::Red.on_default()
            } else {
                white
            };
            writeln!(
                output,
                "{}: {}",
                SummaryToken {
                    value: "status",
                    style: label_style,
                    styled
                },
                SummaryToken {
                    value: status,
                    style: status_style,
                    styled
                }
            )?;
            for field in &fields {
                writeln!(
                    output,
                    "{}: {}",
                    SummaryToken {
                        value: field.label,
                        style: label_style,
                        styled
                    },
                    SummaryToken {
                        value: &field.readable,
                        style: number_style,
                        styled
                    }
                )?;
            }
            writeln!(output, "{}", "-".repeat(width))?;
            if styled {
                write!(output, "{white:#}")?;
            }
            writeln!(output)?;
        }
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
