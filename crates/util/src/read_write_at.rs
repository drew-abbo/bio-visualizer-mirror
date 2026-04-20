//! Exports the [ReadAt] and [WriteAt] traits for reading/writing to a file at a
//! position without seeking there first.

use std::fs::File;
use std::io::{ErrorKind, Result};

/// Allows you to read bytes at a position without seeking there first.
pub trait ReadAt {
    /// Like [std::io::Read::read] but it starts at a byte offset from the start
    /// of the file, rather than where the current cursor is. Whether the cursor
    /// is affected by this is unspecified.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize>;

    /// Like [Self::read_at] but it always reads enough bytes to fill `buf` by
    /// potentially reading multiple times.
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<()> {
        // We'll allow a zero byte read, but not 2 in a row.
        let mut last_was_zero_read = false;

        let mut bytes_read = 0;
        while bytes_read < buf.len() {
            match self.read_at(&mut buf[bytes_read..], offset + bytes_read as u64) {
                Ok(0) if last_was_zero_read => return Err(ErrorKind::UnexpectedEof.into()),
                Err(e) if e.kind() != ErrorKind::Interrupted => return Err(e),

                Ok(0) => last_was_zero_read = true,
                Ok(additional) => {
                    last_was_zero_read = false;
                    bytes_read += additional;
                },
                Err(_) /* ErrorKind::Interrupted */ => {}
            }
        }
        Ok(())
    }
}

/// Allows you to write bytes at a position without seeking there first.
pub trait WriteAt {
    /// Like [std::io::Write::write] but it starts at a byte offset from the
    /// start of the file, rather than where the current cursor is. Whether the
    /// cursor is affected by this is unspecified.
    fn write_at(&self, buf: &[u8], offset: u64) -> Result<usize>;

    /// Like [Self::write_at] but it always writes enough bytes to fill `buf` by
    /// potentially writing multiple times.
    fn write_all_at(&self, buf: &[u8], offset: u64) -> Result<()> {
        let mut bytes_written = 0;
        while bytes_written < buf.len() {
            match self.write_at(&buf[bytes_written..], offset + bytes_written as u64) {
                Ok(additional) => bytes_written += additional,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
mod platform_impl {
    use super::*;
    use std::os::unix::fs::FileExt;

    impl ReadAt for File {
        #[inline(always)]
        fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
            <Self as FileExt>::read_at(self, buf, offset)
        }

        #[inline(always)]
        fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<()> {
            <Self as FileExt>::read_exact_at(self, buf, offset)
        }
    }

    impl WriteAt for File {
        #[inline(always)]
        fn write_at(&self, buf: &[u8], offset: u64) -> Result<usize> {
            <Self as FileExt>::write_at(self, buf, offset)
        }

        #[inline(always)]
        fn write_all_at(&self, buf: &[u8], offset: u64) -> Result<()> {
            <Self as FileExt>::write_all_at(self, buf, offset)
        }
    }
}

#[cfg(windows)]
mod platform_impl {
    use super::*;
    use std::os::windows::fs::FileExt;

    impl ReadAt for File {
        #[inline(always)]
        fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
            <Self as FileExt>::seek_read(self, buf, offset)
        }
    }

    impl WriteAt for File {
        #[inline(always)]
        fn write_at(&self, buf: &[u8], offset: u64) -> Result<usize> {
            <Self as FileExt>::seek_write(self, buf, offset)
        }
    }
}
