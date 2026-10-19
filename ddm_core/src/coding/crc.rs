//! Cyclic redunancy checks
use binrw::io::{self, Read, Seek, SeekFrom, Write};
use crc32fast::Hasher;

/// Writer that automatically updates CRC with written bits
pub struct CrcWriter<'a, W> {
    inner: &'a mut W,
    hasher: Hasher,
}

impl<'a, W> CrcWriter<'a, W> {
    /// Create new CRC writer
    pub fn new(inner: &'a mut W) -> Self {
        Self {
            inner,
            hasher: Hasher::new(),
        }
    }

    /// Finish hashing data and return CRC
    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl<W: Write> Write for CrcWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Seek> Seek for CrcWriter<'_, W> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// Reader that automatically updates CRC with read bits
pub struct CrcReader<'a, R> {
    inner: &'a mut R,
    hasher: Hasher,
}
impl<'a, R> CrcReader<'a, R> {
    /// Create new CRC reader
    pub fn new(inner: &'a mut R) -> Self {
        Self {
            inner,
            hasher: Hasher::new(),
        }
    }

    /// Finish hashing data and return CRC
    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl<R: Read> Read for CrcReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}

impl<R: Seek> Seek for CrcReader<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}
