use std::io::{self, Read, Write};

/// Stable dependency-free generator: do not change without versioning the corpus.
pub struct Corpus {
    state: u64,
    remaining: usize,
    size: usize,
    scenario: &'static str,
    pending: Vec<u8>,
    offset: usize,
    pub chunk: usize,
}
impl Corpus {
    pub fn new(
        seed: u64,
        records: usize,
        size: usize,
        scenario: &'static str,
        chunk: usize,
    ) -> Self {
        assert!(size > 0 && chunk > 0);
        Self {
            state: seed,
            remaining: records,
            size,
            scenario,
            pending: Vec::new(),
            offset: 0,
            chunk,
        }
    }
    fn next(&mut self) {
        self.pending.clear();
        self.offset = 0;
        self.pending.extend_from_slice(b"{\"value\":\"");
        for _ in 0..self.size {
            self.state = self
                .state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Includes raw UTF-8 and escaped characters, without nested recovery candidates.
            let tokens: &[&[u8]] = &[
                b"a",
                b"7",
                b" ",
                b"\\n",
                b"\\\"",
                b"\\\\",
                "é".as_bytes(),
                "🙂".as_bytes(),
            ];
            self.pending
                .extend_from_slice(tokens[(self.state >> 61) as usize]);
        }
        let invalid = self.scenario == "invalid"
            || (self.scenario == "mixed" && self.remaining.is_multiple_of(2));
        self.pending.extend_from_slice(if invalid {
            b"\",\"bad\":truX}\n"
        } else {
            b"\"}\n"
        });
        self.remaining -= 1;
    }
}
impl Read for Corpus {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        if self.offset == self.pending.len() {
            if self.remaining == 0 {
                return Ok(0);
            }
            self.next();
        }
        let count = target
            .len()
            .min(self.chunk)
            .min(self.pending.len() - self.offset);
        target[..count].copy_from_slice(&self.pending[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub bytes: u64,
    pub hash: u64,
}
impl Default for Digest {
    fn default() -> Self {
        Self {
            bytes: 0,
            hash: 14695981039346656037,
        }
    }
}
impl Write for Digest {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for &byte in bytes {
            self.hash = (self.hash ^ byte as u64).wrapping_mul(1099511628211);
        }
        self.bytes += bytes.len() as u64;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
