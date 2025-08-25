//! Utility binary for base64 en-/decoding

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]

use std::{
    fs::File,
    io::{self, prelude::*},
    path::PathBuf,
};

use clap::Parser;

#[derive(Parser)]
struct Opts {
    #[arg(long, short)]
    decode: bool,

    file: Option<PathBuf>,
}

fn main() {
    let Opts { decode, file } = Opts::parse();

    if let Some(file) = file {
        run(File::open(file).unwrap(), decode);
    } else {
        run(io::stdin().lock(), decode);
    }
}

fn run(mut stream: impl Read, decode: bool) {
    let mut out = io::stdout().lock();
    if decode {
        let mut s = String::new();
        stream.read_to_string(&mut s).unwrap();
        io::copy(&mut base64k::Decoder::new(s.chars()), &mut out).unwrap();
    } else {
        let mut enc = base64k::Encoder::<String>::default();
        io::copy(&mut stream, &mut enc).unwrap();
        out.write_all(enc.finish().as_bytes()).unwrap();
    }
}
