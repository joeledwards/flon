use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("flon: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<OsString>) -> io::Result<()> {
    let mut input: Box<dyn Read> = match arguments.as_slice() {
        [] => Box::new(io::stdin()),
        [path] if path == "-" => Box::new(io::stdin()),
        [path] => Box::new(File::open(path)?),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: flon [FILE|-]",
            ));
        }
    };

    let stdout = io::stdout();
    let output = BufWriter::new(stdout.lock());
    let mut output = flon::format(&mut input, output)?;
    output.flush()
}
