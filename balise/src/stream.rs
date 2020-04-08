use std::{
    io,
    io::{Read, Write},
};

/// A Wrapper to combine a `reader` and a `writer` to a Stream.
///
/// Useful to mock a `TcpStream` with two separate `std::io::Cursors`s.
pub struct Stream<R, W>(R, W);

impl<R, W> Stream<R, W>
where
    R: Read,
    W: Write,
{
    /// Create a new Stream.
    ///
    /// The `reader` and `writer` are `std::io::Cursor`s to read and write on the mocked stream.
    pub fn new(reader: R, writer: W) -> Self {
        Self(reader, writer)
    }
}

impl<R, W> Read for Stream<R, W>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf)
    }
}

impl<R, W> Write for Stream<R, W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.1.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.1.flush()
    }
}
