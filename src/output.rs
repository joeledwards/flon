use std::io::{self, BufWriter, LineWriter, Write};

use super::Buffering;

pub(super) enum OutputWriter<W: Write> {
    Buffered(BufWriter<W>),
    Immediate(LineWriter<W>),
}

impl<W: Write> OutputWriter<W> {
    pub(super) fn new(output: W, buffering: Buffering) -> Self {
        match buffering {
            Buffering::Buffered => Self::Buffered(BufWriter::with_capacity(8192, output)),
            Buffering::Immediate => Self::Immediate(LineWriter::with_capacity(8192, output)),
        }
    }
}

impl<W: Write> Write for OutputWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Buffered(output) => output.write(bytes),
            Self::Immediate(output) => {
                // Return at most one line, so write_all also flushes each line
                // when confirmed output commits several lines in one write.
                let end = bytes
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |at| at + 1);
                let count = output.write(&bytes[..end])?;
                if bytes[..count].contains(&b'\n') {
                    output.flush()?;
                }
                Ok(count)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Buffered(output) => output.flush(),
            Self::Immediate(output) => output.flush(),
        }
    }
}
