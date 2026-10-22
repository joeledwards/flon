//! Streaming conversion from JSON to flat object notation.

use std::io::{self, Read, Write};

use json_skimmer::{ContainerKind, ErrorKind, Event, ParseError, Parser, StringKind};

/// Default maximum number of raw JSON bytes retained for one object key.
pub const DEFAULT_MAX_KEY_BYTES: usize = 64 * 1024;

/// Action taken when an object key exceeds [`Config::max_key_bytes`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyOverflowPolicy {
    /// Stop formatting and return an error.
    Error,
    /// Validate the rest of the key and skip its complete associated value.
    SkipValue,
}

/// Action taken when `json-skimmer` reports an invalid JSON candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCandidatePolicy {
    /// Stop formatting and return an error.
    Fail,
    /// Reset candidate state and continue scanning without an annotation.
    Skip,
    /// Reset, continue scanning, and write a JavaScript block comment.
    SkipWithComment,
}

/// Resource limits and overflow behavior for a formatter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    /// Maximum raw bytes retained for one object key.
    pub max_key_bytes: usize,
    pub key_overflow: KeyOverflowPolicy,
    pub invalid_candidate: InvalidCandidatePolicy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_key_bytes: DEFAULT_MAX_KEY_BYTES,
            key_overflow: KeyOverflowPolicy::Error,
            invalid_candidate: InvalidCandidatePolicy::Fail,
        }
    }
}

/// Incrementally translates `json-skimmer` events into FLON assignments.
pub struct Formatter<W> {
    output: W,
    config: Config,
    stack: Vec<Frame>,
    string: Option<ActiveString>,
    number: Option<bool>,
    root_written: bool,
}

#[derive(Debug)]
struct Frame {
    kind: ContainerKind,
    component: Option<Component>,
    key: Option<PendingKey>,
    next_index: u64,
    suppressed: bool,
}

#[derive(Debug)]
enum Component {
    Key(Vec<u8>),
    Index(u64),
}

#[derive(Debug)]
enum PendingKey {
    Buffered(Vec<u8>),
    Skip,
}

enum ActiveString {
    Key(Vec<u8>),
    OverflowedKey,
    DiscardedKey,
    Value { emitted: bool },
}

enum ValueTarget {
    Emit(Component),
    Skip,
}

impl<W: Write> Formatter<W> {
    pub fn new(output: W) -> Self {
        Self::with_config(output, Config::default())
    }

    pub fn with_config(output: W, config: Config) -> Self {
        Self {
            output,
            config,
            stack: Vec::new(),
            string: None,
            number: None,
            root_written: false,
        }
    }

    pub fn into_inner(self) -> W {
        self.output
    }

    /// Handles one parser event. Events from an invalid candidate may already
    /// have produced output; FLON intentionally operates as a live stream.
    pub fn event(&mut self, event: Event<'_>) -> io::Result<()> {
        match event {
            Event::CandidateStart { .. } => self.reset_candidate(),
            Event::ObjectStart { .. } => self.open(ContainerKind::Object)?,
            Event::ArrayStart { .. } => self.open(ContainerKind::Array)?,
            Event::ObjectEnd { .. } => self.close(ContainerKind::Object)?,
            Event::ArrayEnd { .. } => self.close(ContainerKind::Array)?,
            Event::StringStart {
                kind: StringKind::Key,
                ..
            } => {
                let frame = self.stack.last().ok_or_else(invalid_events)?;
                if frame.kind != ContainerKind::Object {
                    return Err(invalid_events());
                }
                self.string = Some(if frame.suppressed {
                    ActiveString::DiscardedKey
                } else {
                    ActiveString::Key(Vec::new())
                });
            }
            Event::StringStart {
                kind: StringKind::Value,
                ..
            } => {
                let emitted = self.begin_scalar()?;
                if emitted {
                    self.output.write_all(b"\"")?;
                }
                self.string = Some(ActiveString::Value { emitted });
            }
            Event::StringFragment { kind, bytes, .. } => self.string_fragment(kind, bytes)?,
            Event::StringEnd { kind, .. } => self.end_string(kind)?,
            Event::NumberStart { .. } => {
                self.number = Some(self.begin_scalar()?);
            }
            Event::NumberFragment { bytes, .. } => match self.number {
                Some(true) => self.output.write_all(bytes)?,
                Some(false) => {}
                None => return Err(invalid_events()),
            },
            Event::NumberEnd { .. } => match self.number.take() {
                Some(true) => self.output.write_all(b";\n")?,
                Some(false) => {}
                None => return Err(invalid_events()),
            },
            Event::Boolean { value, .. } => {
                self.scalar(if value { b"true" } else { b"false" }, false)?;
            }
            Event::Null { .. } => self.scalar(b"null", false)?,
            Event::Invalid(error) => self.invalid_candidate(error)?,
            Event::CandidateEnd { .. } => self.reset_candidate(),
        }
        Ok(())
    }

    fn reset_candidate(&mut self) {
        self.stack.clear();
        self.string = None;
        self.number = None;
    }

    fn invalid_candidate(&mut self, error: ParseError) -> io::Result<()> {
        match self.config.invalid_candidate {
            InvalidCandidatePolicy::Fail => return Err(parse_error(&error)),
            InvalidCandidatePolicy::Skip => {}
            InvalidCandidatePolicy::SkipWithComment => {
                let partial_scalar =
                    matches!(self.string, Some(ActiveString::Value { emitted: true }))
                        || self.number == Some(true);
                if partial_scalar {
                    self.output.write_all(b"\n")?;
                }
                self.output.write_all(b"/* flon-error kind=")?;
                self.output
                    .write_all(error_kind_name(error.kind).as_bytes())?;
                write!(self.output, " offset={}", error.offset)?;
                if let Some(start) = error.candidate_start {
                    write!(self.output, " candidate_start={start}")?;
                } else {
                    self.output.write_all(b" candidate_start=none")?;
                }
                self.output.write_all(b" */\n")?;
            }
        }
        self.reset_candidate();
        Ok(())
    }

    fn open(&mut self, kind: ContainerKind) -> io::Result<()> {
        let component = if self.stack.is_empty() {
            if !self.root_written {
                self.output.write_all(b"json = ")?;
                self.write_container(kind)?;
                self.output.write_all(b";\n")?;
                self.root_written = true;
            }
            None
        } else {
            match self.take_target()? {
                ValueTarget::Emit(component) => {
                    self.write_path(Some(&component))?;
                    self.output.write_all(b" = ")?;
                    self.write_container(kind)?;
                    self.output.write_all(b";\n")?;
                    Some(component)
                }
                ValueTarget::Skip => None,
            }
        };

        let suppressed = !self.stack.is_empty() && component.is_none();

        self.stack.push(Frame {
            kind,
            component,
            key: None,
            next_index: 0,
            suppressed,
        });
        Ok(())
    }

    fn close(&mut self, expected: ContainerKind) -> io::Result<()> {
        let frame = self.stack.pop().ok_or_else(invalid_events)?;
        if frame.kind != expected {
            return Err(invalid_events());
        }
        Ok(())
    }

    fn end_string(&mut self, kind: StringKind) -> io::Result<()> {
        match (kind, self.string.take()) {
            (StringKind::Key, Some(ActiveString::Key(token))) => {
                let frame = self.stack.last_mut().ok_or_else(invalid_events)?;
                if frame.kind != ContainerKind::Object {
                    return Err(invalid_events());
                }
                frame.key = Some(PendingKey::Buffered(token));
            }
            (StringKind::Key, Some(ActiveString::OverflowedKey | ActiveString::DiscardedKey)) => {
                let frame = self.stack.last_mut().ok_or_else(invalid_events)?;
                if frame.kind != ContainerKind::Object {
                    return Err(invalid_events());
                }
                frame.key = Some(PendingKey::Skip);
            }
            (StringKind::Value, Some(ActiveString::Value { emitted: true })) => {
                self.output.write_all(b"\";\n")?;
            }
            (StringKind::Value, Some(ActiveString::Value { emitted: false })) => {}
            _ => return Err(invalid_events()),
        }
        Ok(())
    }

    fn string_fragment(&mut self, kind: StringKind, bytes: &[u8]) -> io::Result<()> {
        match (kind, self.string.as_mut()) {
            (StringKind::Key, Some(ActiveString::Key(token))) => {
                if bytes.len() <= self.config.max_key_bytes.saturating_sub(token.len()) {
                    token.extend_from_slice(bytes);
                } else {
                    match self.config.key_overflow {
                        KeyOverflowPolicy::Error => return Err(key_overflow()),
                        KeyOverflowPolicy::SkipValue => {
                            self.string = Some(ActiveString::OverflowedKey);
                        }
                    }
                }
            }
            (StringKind::Key, Some(ActiveString::OverflowedKey | ActiveString::DiscardedKey)) => {}
            (StringKind::Value, Some(ActiveString::Value { emitted: true })) => {
                self.output.write_all(bytes)?;
            }
            (StringKind::Value, Some(ActiveString::Value { emitted: false })) => {}
            _ => return Err(invalid_events()),
        }
        Ok(())
    }

    fn scalar(&mut self, value: &[u8], quoted: bool) -> io::Result<()> {
        if !self.begin_scalar()? {
            return Ok(());
        }
        if quoted {
            self.output.write_all(b"\"")?;
        }
        self.output.write_all(value)?;
        if quoted {
            self.output.write_all(b"\"")?;
        }
        self.output.write_all(b";\n")
    }

    fn begin_scalar(&mut self) -> io::Result<bool> {
        match self.take_target()? {
            ValueTarget::Emit(component) => {
                self.write_path(Some(&component))?;
                self.output.write_all(b" = ")?;
                Ok(true)
            }
            ValueTarget::Skip => Ok(false),
        }
    }

    fn take_target(&mut self) -> io::Result<ValueTarget> {
        let frame = self.stack.last_mut().ok_or_else(invalid_events)?;
        if frame.suppressed {
            frame.key = None;
            return Ok(ValueTarget::Skip);
        }
        match frame.kind {
            ContainerKind::Object => match frame.key.take().ok_or_else(invalid_events)? {
                PendingKey::Buffered(key) => Ok(ValueTarget::Emit(Component::Key(key))),
                PendingKey::Skip => Ok(ValueTarget::Skip),
            },
            ContainerKind::Array => {
                let index = frame.next_index;
                frame.next_index = frame.next_index.checked_add(1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "array index overflow")
                })?;
                Ok(ValueTarget::Emit(Component::Index(index)))
            }
        }
    }

    fn write_path(&mut self, tail: Option<&Component>) -> io::Result<()> {
        self.output.write_all(b"json")?;
        for component in self
            .stack
            .iter()
            .filter_map(|frame| frame.component.as_ref())
        {
            write_component(&mut self.output, component)?;
        }
        if let Some(component) = tail {
            write_component(&mut self.output, component)?;
        }
        Ok(())
    }

    fn write_container(&mut self, kind: ContainerKind) -> io::Result<()> {
        self.output.write_all(match kind {
            ContainerKind::Object => b"{}",
            ContainerKind::Array => b"[]",
        })
    }
}

fn write_component(output: &mut impl Write, component: &Component) -> io::Result<()> {
    match component {
        Component::Index(index) => write!(output, "[{index}]"),
        Component::Key(key) if is_identifier(key) => {
            output.write_all(b".")?;
            output.write_all(key)
        }
        Component::Key(key) => {
            output.write_all(b"[\"")?;
            output.write_all(key)?;
            output.write_all(b"\"]")
        }
    }
}

fn is_identifier(key: &[u8]) -> bool {
    let Some((&first, rest)) = key.split_first() else {
        return false;
    };
    matches!(first, b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$')
        && rest
            .iter()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$'))
}

fn invalid_events() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "inconsistent json-skimmer event sequence",
    )
}

fn key_overflow() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "object key exceeds configured byte limit",
    )
}

fn parse_error(error: &ParseError) -> io::Error {
    let candidate = error
        .candidate_start
        .map_or_else(|| "none".to_owned(), |start| start.to_string());
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "invalid JSON candidate: {} at byte {} (candidate start: {candidate})",
            error_kind_name(error.kind),
            error.offset
        ),
    )
}

fn error_kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::UnexpectedByte => "unexpected_byte",
        ErrorKind::InvalidEscape => "invalid_escape",
        ErrorKind::InvalidUnicodeEscape => "invalid_unicode_escape",
        ErrorKind::ControlCharacterInString => "control_character_in_string",
        ErrorKind::InvalidNumber => "invalid_number",
        ErrorKind::UnexpectedEnd => "unexpected_end",
        ErrorKind::ExpectedSingleValue => "expected_single_value",
        ErrorKind::TrailingData => "trailing_data",
    }
}

/// Reads a stream to EOF and writes plain FLON output.
pub fn format<R: Read, W: Write>(mut input: R, output: W) -> io::Result<W> {
    format_with_config(&mut input, output, Config::default())
}

/// Reads a stream to EOF using the supplied formatter configuration.
pub fn format_with_config<R: Read, W: Write>(
    mut input: R,
    output: W,
    config: Config,
) -> io::Result<W> {
    let mut parser = Parser::default();
    let mut formatter = Formatter::with_config(output, config);
    let mut buffer = [0_u8; 8 * 1024];

    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let mut error = None;
        parser.push(&buffer[..count], |event| {
            if error.is_none() {
                error = formatter.event(event).err();
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
    }

    let mut error = None;
    parser.finish(|event| error = formatter.event(event).err());
    if let Some(error) = error {
        return Err(error);
    }
    Ok(formatter.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flon(input: &[u8]) -> String {
        let output = format(input, Vec::new()).expect("format JSON");
        String::from_utf8(output).expect("UTF-8 FLON output")
    }

    fn flon_in_chunks(input: &[u8], chunk_size: usize) -> String {
        let mut parser = Parser::default();
        let mut formatter = Formatter::new(Vec::new());
        for chunk in input.chunks(chunk_size) {
            parser.push(chunk, |event| formatter.event(event).expect("format event"));
        }
        parser.finish(|event| formatter.event(event).expect("finish formatter"));
        String::from_utf8(formatter.into_inner()).expect("UTF-8 FLON output")
    }

    fn configured(input: &[u8], max_key_bytes: usize, key_overflow: KeyOverflowPolicy) -> String {
        let output = format_with_config(
            input,
            Vec::new(),
            Config {
                max_key_bytes,
                key_overflow,
                ..Config::default()
            },
        )
        .expect("format configured JSON");
        String::from_utf8(output).expect("UTF-8 FLON output")
    }

    #[test]
    fn formats_scalar_object_members() {
        assert_eq!(
            flon(br#"{"name":"flon","count":1,"ready":true,"missing":null}"#),
            "json = {};\njson.name = \"flon\";\njson.count = 1;\njson.ready = true;\njson.missing = null;\n"
        );
    }

    #[test]
    fn formats_nested_arrays_and_objects_in_encounter_order() {
        assert_eq!(
            flon(br#"{"records":[{"id":1},[],[2]]}"#),
            "json = {};\n\
             json.records = [];\n\
             json.records[0] = {};\n\
             json.records[0].id = 1;\n\
             json.records[1] = [];\n\
             json.records[2] = [];\n\
             json.records[2][0] = 2;\n"
        );
    }

    #[test]
    fn uses_brackets_for_non_identifier_keys_and_preserves_escapes() {
        assert_eq!(
            flon(br#"{"hyphen-ate":"quoted\nvalue","\"q\"":1}"#),
            "json = {};\njson[\"hyphen-ate\"] = \"quoted\\nvalue\";\njson[\"\\\"q\\\"\"] = 1;\n"
        );
    }

    #[test]
    fn formats_root_arrays() {
        assert_eq!(
            flon(br#"[1,"two"]"#),
            "json = [];\njson[0] = 1;\njson[1] = \"two\";\n"
        );
    }

    #[test]
    fn retains_only_one_root_declaration_for_consecutive_documents() {
        assert_eq!(
            flon(b"{\"a\":1}\n{\"b\":2}"),
            "json = {};\njson.a = 1;\njson.b = 2;\n"
        );
    }

    #[test]
    fn recovers_documents_from_junk_across_single_byte_chunks() {
        assert_eq!(
            flon_in_chunks(b"junk {\"a\":1} more junk [true]", 1),
            "json = {};\njson.a = 1;\njson[0] = true;\n"
        );
    }

    #[test]
    fn default_key_limit_is_64_kib_and_errors_on_overflow() {
        assert_eq!(DEFAULT_MAX_KEY_BYTES, 64 * 1024);
        assert_eq!(Config::default().key_overflow, KeyOverflowPolicy::Error);

        let key = "a".repeat(DEFAULT_MAX_KEY_BYTES + 1);
        let input = format!(r#"{{"{key}":1}}"#);
        let error = format(input.as_bytes(), Vec::new()).expect_err("reject oversized key");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "object key exceeds configured byte limit"
        );
    }

    #[test]
    fn accepts_a_key_at_the_configured_limit() {
        assert_eq!(
            configured(br#"{"abcd":1}"#, 4, KeyOverflowPolicy::Error),
            "json = {};\njson.abcd = 1;\n"
        );
    }

    #[test]
    fn skips_oversized_scalar_and_container_values_then_resumes() {
        assert_eq!(
            configured(
                br#"{"ok":1,"large-key":{"nested":[1,{"ignored":2}]},"wide-key":"ignored","after":3}"#,
                5,
                KeyOverflowPolicy::SkipValue,
            ),
            "json = {};\njson.ok = 1;\njson.after = 3;\n"
        );
    }

    #[test]
    fn validates_an_oversized_key_and_recovers_the_next_candidate() {
        let output = format_with_config(
            br#"{"oversized\q":1} {"after":2}"#.as_slice(),
            Vec::new(),
            Config {
                max_key_bytes: 5,
                key_overflow: KeyOverflowPolicy::SkipValue,
                invalid_candidate: InvalidCandidatePolicy::Skip,
            },
        )
        .expect("recover invalid oversized key");
        assert_eq!(
            String::from_utf8(output).expect("UTF-8 FLON output"),
            "json = {};\njson.after = 2;\n"
        );
    }

    #[test]
    fn skips_an_oversized_key_across_single_byte_chunks() {
        let mut parser = Parser::default();
        let mut formatter = Formatter::with_config(
            Vec::new(),
            Config {
                max_key_bytes: 3,
                key_overflow: KeyOverflowPolicy::SkipValue,
                ..Config::default()
            },
        );
        for byte in br#"{"oversized":[1,2],"ok":3}"#.chunks(1) {
            parser.push(byte, |event| formatter.event(event).expect("format event"));
        }
        parser.finish(|event| formatter.event(event).expect("finish formatter"));

        assert_eq!(
            String::from_utf8(formatter.into_inner()).expect("UTF-8 FLON output"),
            "json = {};\njson.ok = 3;\n"
        );
    }

    #[test]
    fn invalid_candidates_fail_by_default() {
        assert_eq!(
            Config::default().invalid_candidate,
            InvalidCandidatePolicy::Fail
        );
        let error = format(br#"{"bad":truX}"#.as_slice(), Vec::new())
            .expect_err("reject invalid candidate");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "invalid JSON candidate: unexpected_byte at byte 10 (candidate start: 0)"
        );
    }

    #[test]
    fn can_skip_an_invalid_candidate_silently() {
        assert_eq!(
            configured_invalid(br#"{"bad":truX} {"ok":1}"#, InvalidCandidatePolicy::Skip,),
            "json = {};\njson.ok = 1;\n"
        );
    }

    #[test]
    fn can_annotate_an_invalid_candidate_and_continue() {
        assert_eq!(
            configured_invalid(
                br#"{"bad":truX} {"ok":1}"#,
                InvalidCandidatePolicy::SkipWithComment,
            ),
            "json = {};\n\
             /* flon-error kind=unexpected_byte offset=10 candidate_start=0 */\n\
             json.ok = 1;\n"
        );
    }

    fn configured_invalid(input: &[u8], invalid_candidate: InvalidCandidatePolicy) -> String {
        let output = format_with_config(
            input,
            Vec::new(),
            Config {
                invalid_candidate,
                ..Config::default()
            },
        )
        .expect("format with invalid-candidate policy");
        String::from_utf8(output).expect("UTF-8 FLON output")
    }
}
