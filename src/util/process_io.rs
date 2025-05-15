use std::io;
use std::sync::{Arc, Mutex};

pub trait ProcessIO {
    type OW: io::Write;
    type EW: io::Write;

    fn out(&self) -> Self::OW;
    fn err(&self) -> Self::EW;
}

enum Output {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

#[derive(Clone)]
pub struct IOBuffer {
    buffer: Arc<Mutex<Vec<Output>>>
}

#[derive(Clone)]
pub struct IOBufferOut {
    buffer: Arc<Mutex<Vec<Output>>>
}

#[derive(Clone)]
pub struct IOBufferErr {
    buffer: Arc<Mutex<Vec<Output>>>
}

impl IOBuffer {
    pub fn new() -> IOBuffer {
        IOBuffer { buffer: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let mut buffer_lock = self.buffer.lock().unwrap();
        let mut out = io::stdout().lock();
        let mut err = io::stderr().lock();
        for output in buffer_lock.drain(..) {
            match output {
                Output::Stdout(buf) => {
                    io::Write::write(&mut out, buf.as_slice())?;
                }
                Output::Stderr(buf) => {
                    io::Write::write(&mut err, buf.as_slice())?;
                }
            }
        }
        Ok(())
    }
}

impl ProcessIO for IOBuffer {
    type OW = IOBufferOut;
    type EW = IOBufferErr;

    fn out(&self) -> IOBufferOut {
        IOBufferOut { buffer: self.buffer.clone() }
    }

    fn err(&self) -> IOBufferErr {
        IOBufferErr { buffer: self.buffer.clone() }
    }
}

impl io::Write for IOBufferOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer_lock = self.buffer.lock().unwrap();
        buffer_lock.push(Output::Stdout(Vec::from(buf)));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Write for IOBufferErr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer_lock = self.buffer.lock().unwrap();
        buffer_lock.push(Output::Stderr(Vec::from(buf)));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct StandardIO;

impl ProcessIO for StandardIO {
    type OW = io::Stdout;
    type EW = io::Stderr;

    fn out(&self) -> io::Stdout {
        io::stdout()
    }

    fn err(&self) -> io::Stderr {
        io::stderr()
    }
}

