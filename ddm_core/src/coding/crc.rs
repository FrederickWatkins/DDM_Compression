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
        Self::new_with_prefix(inner, &[])
    }

    /// Create a CRC writer, including bytes that have already been written in the checksum.
    ///
    /// This is useful when a framing layer writes a header before the payload writer is
    /// constructed, but the protocol checksum covers both the framing header and payload.
    pub fn new_with_prefix(inner: &'a mut W, prefix: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(prefix);
        Self { inner, hasher }
    }

    /// Finish hashing data and return CRC
    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl<W: Write> Write for CrcWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        let written_buf = buf.get(..written).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "writer reported an invalid byte count",
            )
        })?;
        self.hasher.update(written_buf);
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
        Self::new_with_prefix(inner, &[])
    }

    /// Create a CRC reader, including bytes that have already been read by a framing layer.
    ///
    /// This is useful when a framing layer consumes a header before the payload reader is
    /// constructed, but the protocol checksum covers both the framing header and payload.
    pub fn new_with_prefix(inner: &'a mut R, prefix: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(prefix);
        Self { inner, hasher }
    }

    /// Finish hashing data and return CRC
    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl<R: Read> Read for CrcReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        let read_buf = buf.get(..read).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "reader reported an invalid byte count",
            )
        })?;
        self.hasher.update(read_buf);
        Ok(read)
    }
}

impl<R: Seek> Seek for CrcReader<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}