// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

pub(crate) mod combined_record;
pub(crate) mod compressed;
pub(crate) mod entry;
pub(crate) mod hashed;
pub(crate) mod locator;
pub(crate) mod owned;

pub use combined_record::CombinedCentralDirectoryRecord;

use crate::string::{StringEncoding, ZipString};
use futures_lite::io::{AsyncRead, AsyncReadExt};

/// Maximum NUL padding after the declared archive comment.
const MAX_TRAILING_NUL_BYTES: usize = 4096;

/// Read and return a dynamic length string from a reader which impls AsyncRead.
pub(crate) async fn read_string<R>(reader: R, length: usize, encoding: StringEncoding) -> std::io::Result<ZipString>
where
    R: AsyncRead + Unpin,
{
    Ok(ZipString::new(read_bytes(reader, length).await?, encoding))
}

/// Read and return a dynamic length vector of bytes from a reader which impls AsyncRead.
pub(crate) async fn read_bytes<R>(reader: R, length: usize) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut buffer = Vec::with_capacity(length);
    let read = reader.take(length as u64).read_to_end(&mut buffer).await?;
    if read != length {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "ZIP structure was truncated"));
    }

    Ok(buffer)
}

/// Skip a specified number of bytes in an AsyncRead implementer.
pub(crate) async fn skip_bytes<R>(mut reader: R, length: u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut buf = [0u8; 8192];
    let mut remaining = length;
    while remaining > 0 {
        let to_read = std::cmp::min(remaining, buf.len() as u64) as usize;
        let n = reader.read(&mut buf[..to_read]).await?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "ZIP structure was truncated"));
        }
        remaining -= n as u64;
    }
    Ok(())
}

/// Requires EOF after the archive comment, tolerating at most 4 KiB of NUL padding.
pub(crate) async fn validate_trailing_contents<R: AsyncRead + Unpin>(reader: R) -> crate::error::Result<()> {
    // Read one byte beyond the limit to distinguish an exact-limit suffix from excess padding.
    let mut reader = reader.take((MAX_TRAILING_NUL_BYTES + 1) as u64);
    let mut buffer = [0; 8192];
    let mut padding = 0;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        padding += read;
        if padding > MAX_TRAILING_NUL_BYTES || buffer[..read].iter().any(|&byte| byte != 0) {
            return Err(crate::error::ZipError::TrailingContents);
        }
    }
}

/// Conservatively excludes embedded ZIP records from the archive comment, matching uv's policy.
/// Entry comments and file contents are not subject to this restriction.
pub(crate) fn validate_archive_comment(comment: &ZipString) -> crate::error::Result<()> {
    if comment.as_bytes().iter().any(|byte| (1..=8).contains(byte)) {
        return Err(crate::error::ZipError::ZipInZip);
    }
    Ok(())
}

/// A macro that returns the inner value of an Ok or early-returns in the case of an Err.
///
/// This is almost identical to the ? operator but handles the situation when a Result is used in combination with
/// Poll (eg. tokio's IO traits such as AsyncRead).
macro_rules! poll_result_ok {
    ($poll:expr) => {
        match $poll {
            Ok(inner) => inner,
            Err(err) => return Poll::Ready(Err(err)),
        }
    };
}

use poll_result_ok;
