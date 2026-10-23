use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    flushed: Vec<usize>,
    failure: Option<io::ErrorKind>,
}

#[derive(Clone, Default)]
struct Probe(Rc<RefCell<State>>);

impl Write for Probe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Exercise partial writes below both buffering policies.
        let count = bytes.len().min(2);
        self.0.borrow_mut().bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.0.borrow_mut();
        let len = state.bytes.len();
        state.flushed.push(len);
        if let Some(kind) = state.failure {
            Err(io::Error::new(kind, "flush failure"))
        } else {
            Ok(())
        }
    }
}

fn effective(buffering: Buffering, output_mode: OutputMode) -> EffectiveConfig {
    EffectiveConfig {
        formatter: Config {
            output_mode,
            ..Config::default()
        },
        diagnostics: Diagnostics::Quiet,
        color: ColorChoice::Never,
        buffering,
        summary: false,
        summary_format: crate::summary::SummaryFormat::Text,
    }
}

#[test]
fn buffering_options_are_explicit_and_independent_of_profiles_and_validation() {
    let parse = |args: &[&str]| {
        Cli::try_parse_from(std::iter::once("flon").chain(args.iter().copied())).unwrap()
    };
    assert_eq!(resolve(&parse(&[]), false).buffering, Buffering::Buffered);
    for args in [
        vec!["--buffering", "immediate"],
        vec!["--no-buffer"],
        vec!["-B"],
    ] {
        assert_eq!(
            resolve(&parse(&args), false).buffering,
            Buffering::Immediate
        );
    }
    for profile in ["wraith", "infinite"] {
        for choice in ["buffered", "immediate"] {
            let config = resolve(
                &parse(&["--profile", profile, "--buffering", choice]),
                false,
            );
            assert_eq!(
                config.buffering == Buffering::Immediate,
                choice == "immediate"
            );
            assert_eq!(
                matches!(config.formatter.output_mode, OutputMode::Confirmed(_)),
                profile == "infinite"
            );
        }
    }
    for args in [
        vec!["flon", "--buffering", "other"],
        vec!["flon", "--no-buffer", "--buffering", "buffered"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn immediate_flushes_each_completed_line_and_final_partial_line() {
    let probe = Probe::default();
    let mut writer = OutputWriter::new(probe.clone(), Buffering::Immediate);
    writer.write_all(b"first").unwrap();
    assert!(probe.0.borrow().flushed.is_empty());
    writer.write_all(b"\nsecond\npartial").unwrap();
    assert_eq!(probe.0.borrow().flushed, vec![6, 13]);
    writer.flush().unwrap();
    assert_eq!(probe.0.borrow().bytes, b"first\nsecond\npartial");
    assert_eq!(probe.0.borrow().flushed, vec![6, 13, 20]);
}

#[test]
fn buffered_defers_small_output_until_final_flush() {
    let probe = Probe::default();
    let mut writer = OutputWriter::new(probe.clone(), Buffering::Buffered);
    writer.write_all(b"first\nsecond\n").unwrap();
    assert!(probe.0.borrow().bytes.is_empty());
    writer.flush().unwrap();
    assert_eq!(probe.0.borrow().bytes, b"first\nsecond\n");
    assert_eq!(probe.0.borrow().flushed, vec![13]);
}

// Observe the real CLI stream adapter before its next input read, without a
// subprocess, terminal, or integration test.
struct ObservedInput {
    probe: Probe,
    step: usize,
    confirmed: bool,
    buffering: Buffering,
}

impl Read for ObservedInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let chunk = match self.step {
            0 => b"[1,".as_slice(),
            1 => {
                let state = self.probe.0.borrow();
                if self.confirmed || self.buffering == Buffering::Buffered {
                    assert!(state.bytes.is_empty());
                } else {
                    assert_eq!(state.bytes, b"json = [];\njson[0] = 1;\n");
                    assert_eq!(state.flushed.len(), 2);
                }
                b"2]"
            }
            _ => return Ok(0),
        };
        buffer[..chunk.len()].copy_from_slice(chunk);
        self.step += 1;
        Ok(chunk.len())
    }
}

#[test]
fn live_and_confirmed_output_respect_both_buffering_policies() {
    for buffering in [Buffering::Buffered, Buffering::Immediate] {
        for output_mode in [
            OutputMode::Live,
            OutputMode::Confirmed(ConfirmedOutput {
                memory_threshold: 0,
                ..ConfirmedOutput::default()
            }),
        ] {
            let probe = Probe::default();
            let input = ObservedInput {
                probe: probe.clone(),
                step: 0,
                confirmed: matches!(output_mode, OutputMode::Confirmed(_)),
                buffering,
            };
            write_stream(input, probe.clone(), effective(buffering, output_mode)).unwrap();
            let state = probe.0.borrow();
            assert_eq!(state.bytes, b"json = [];\njson[0] = 1;\njson[1] = 2;\n");
            assert_eq!(
                state.flushed.len(),
                if buffering == Buffering::Immediate {
                    4
                } else {
                    1
                }
            );
        }
    }
}

#[test]
fn flush_errors_propagate_including_broken_pipe() {
    for buffering in [Buffering::Buffered, Buffering::Immediate] {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
            let probe = Probe::default();
            probe.0.borrow_mut().failure = Some(kind);
            let error = write_stream(
                b"[]".as_slice(),
                probe,
                effective(buffering, OutputMode::Live),
            )
            .unwrap_err();
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), "flush failure");
        }
    }
}

#[test]
fn large_lines_preserve_bytes_with_bounded_buffers() {
    let input = [vec![b'x'; 20_000], b"\nend".to_vec()].concat();
    for buffering in [Buffering::Buffered, Buffering::Immediate] {
        let probe = Probe::default();
        let mut writer = OutputWriter::new(probe.clone(), buffering);
        writer.write_all(&input).unwrap();
        writer.flush().unwrap();
        assert_eq!(probe.0.borrow().bytes, input);
    }
}

#[test]
fn successful_eof_flushes_live_partial_output_but_confirmed_discards_it() {
    for buffering in [Buffering::Buffered, Buffering::Immediate] {
        for confirmed in [false, true] {
            let probe = Probe::default();
            let mode = if confirmed {
                OutputMode::Confirmed(ConfirmedOutput::default())
            } else {
                OutputMode::Live
            };
            let mut config = effective(buffering, mode);
            config.formatter.invalid_candidate = InvalidCandidatePolicy::Skip;
            write_stream(br#"["unfinished"#.as_slice(), probe.clone(), config).unwrap();
            let state = probe.0.borrow();
            assert_eq!(
                state.bytes,
                if confirmed {
                    b"".as_slice()
                } else {
                    b"json = [];\njson[0] = \"unfinished"
                }
            );
            assert_eq!(state.flushed.last(), Some(&state.bytes.len()));
        }
    }
}
