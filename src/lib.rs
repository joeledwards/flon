//! Streaming conversion from JSON to flat object notation.

use std::io::{self, Read, Write};

use json_skimmer::{ContainerKind, Event, Parser, StringKind};

/// Incrementally translates `json-skimmer` events into FLON assignments.
pub struct Formatter<W> {
    output: W,
    stack: Vec<Frame>,
    string: Option<ActiveString>,
    number: bool,
    root_written: bool,
}

#[derive(Debug)]
struct Frame {
    kind: ContainerKind,
    component: Option<Component>,
    key: Option<Vec<u8>>,
    next_index: u64,
}

#[derive(Debug)]
enum Component {
    Key(Vec<u8>),
    Index(u64),
}

enum ActiveString {
    Key(Vec<u8>),
    Value,
}

impl<W: Write> Formatter<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            stack: Vec::new(),
            string: None,
            number: false,
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
            Event::StringStart { kind: StringKind::Key, .. } => {
                self.string = Some(ActiveString::Key(Vec::new()));
            }
            Event::StringStart { kind: StringKind::Value, .. } => {
                self.begin_scalar()?;
                self.output.write_all(b"\"")?;
                self.string = Some(ActiveString::Value);
            }
            Event::StringFragment { kind, bytes, .. } => {
                match (kind, self.string.as_mut()) {
                    (StringKind::Key, Some(ActiveString::Key(token))) => token.extend_from_slice(bytes),
                    (StringKind::Value, Some(ActiveString::Value)) => self.output.write_all(bytes)?,
                    _ => return Err(invalid_events()),
                }
            }
            Event::StringEnd { kind, .. } => self.end_string(kind)?,
            Event::NumberStart { .. } => {
                self.begin_scalar()?;
                self.number = true;
            }
            Event::NumberFragment { bytes, .. } if self.number => self.output.write_all(bytes)?,
            Event::NumberFragment { .. } => return Err(invalid_events()),
            Event::NumberEnd { .. } => {
                if !self.number { return Err(invalid_events()); }
                self.number = false;
                self.output.write_all(b";\n")?;
            }
            Event::Boolean { value, .. } => {
                self.scalar(if value { b"true" } else { b"false" }, false)?;
            }
            Event::Null { .. } => self.scalar(b"null", false)?,
            Event::Invalid(_) => self.reset_candidate(),
            Event::CandidateEnd { .. } => self.reset_candidate(),
        }
        Ok(())
    }

    fn reset_candidate(&mut self) {
        self.stack.clear();
        self.string = None;
        self.number = false;
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
            let component = self.take_component()?;
            self.write_path(Some(&component))?;
            self.output.write_all(b" = ")?;
            self.write_container(kind)?;
            self.output.write_all(b";\n")?;
            Some(component)
        };

        self.stack.push(Frame { kind, component, key: None, next_index: 0 });
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
                frame.key = Some(token);
            }
            (StringKind::Value, Some(ActiveString::Value)) => self.output.write_all(b"\";\n")?,
            _ => return Err(invalid_events()),
        }
        Ok(())
    }

    fn scalar(&mut self, value: &[u8], quoted: bool) -> io::Result<()> {
        self.begin_scalar()?;
        if quoted { self.output.write_all(b"\"")?; }
        self.output.write_all(value)?;
        if quoted { self.output.write_all(b"\"")?; }
        self.output.write_all(b";\n")
    }

    fn begin_scalar(&mut self) -> io::Result<()> {
        let component = self.take_component()?;
        self.write_path(Some(&component))?;
        self.output.write_all(b" = ")
    }

    fn take_component(&mut self) -> io::Result<Component> {
        let frame = self.stack.last_mut().ok_or_else(invalid_events)?;
        match frame.kind {
            ContainerKind::Object => frame.key.take().map(Component::Key).ok_or_else(invalid_events),
            ContainerKind::Array => {
                let index = frame.next_index;
                frame.next_index = frame.next_index.checked_add(1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "array index overflow")
                })?;
                Ok(Component::Index(index))
            }
        }
    }

    fn write_path(&mut self, tail: Option<&Component>) -> io::Result<()> {
        self.output.write_all(b"json")?;
        for component in self.stack.iter().filter_map(|frame| frame.component.as_ref()) {
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
    let Some((&first, rest)) = key.split_first() else { return false; };
    matches!(first, b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$')
        && rest.iter().all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$'))
}

fn invalid_events() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "inconsistent json-skimmer event sequence")
}

/// Reads a stream to EOF and writes plain FLON output.
pub fn format<R: Read, W: Write>(mut input: R, output: W) -> io::Result<W> {
    let mut parser = Parser::default();
    let mut formatter = Formatter::new(output);
    let mut buffer = [0_u8; 8 * 1024];

    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 { break; }
        let mut error = None;
        parser.push(&buffer[..count], |event| {
            if error.is_none() {
                error = formatter.event(event).err();
            }
        });
        if let Some(error) = error { return Err(error); }
    }

    let mut error = None;
    parser.finish(|event| error = formatter.event(event).err());
    if let Some(error) = error { return Err(error); }
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
        assert_eq!(flon(br#"[1,"two"]"#), "json = [];\njson[0] = 1;\njson[1] = \"two\";\n");
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
}
