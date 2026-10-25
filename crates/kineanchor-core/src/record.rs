// SPDX-License-Identifier: GPL-3.0-or-later

use crate::protocol::{Message, decode_message, encode_message};
use crate::types::MotionSample;
use std::io::{BufRead, Write};

pub struct Recorder<W: Write> {
    writer: W,
}

impl<W: Write> Recorder<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn record(&mut self, sample: &MotionSample) -> std::io::Result<()> {
        let msg = Message::from(*sample);
        let line = encode_message(&msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.writer.write_all(line.as_bytes())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub struct Player<R: BufRead> {
    reader: R,
}

impl<R: BufRead> Player<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn next_sample(&mut self) -> Option<Result<MotionSample, std::io::Error>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => {
                let msg = match decode_message(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        return Some(Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)));
                    }
                };
                match MotionSample::try_from(msg) {
                    Ok(sample) => Some(Ok(sample)),
                    Err(e) => Some(Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))),
                }
            }
            Err(e) => Some(Err(e)),
        }
    }
}
