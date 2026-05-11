// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use futures_lite::io::AsyncWrite;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::base::write::{central_directory_size_field, ZipFileWriter};
use crate::error::{Zip64ErrorCase, ZipError};
use crate::spec::consts::NON_ZIP64_MAX_SIZE;

pub(crate) mod offset;
mod zip64;

/// /dev/null for AsyncWrite.
/// Useful for tests that involve writing, but not reading, large amounts of data.
pub(crate) struct AsyncSink;

// AsyncSink is always ready to receive bytes and throw them away.
impl AsyncWrite for AsyncSink {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn reject_large_archive_comment() {
    let mut buffer = Vec::new();
    let mut writer = ZipFileWriter::new(&mut buffer);
    writer.comment("x".repeat(u16::MAX as usize + 1));

    let result = writer.close().await;

    assert!(matches!(result, Err(ZipError::CommentTooLarge)));
    assert!(buffer.is_empty());
}

#[test]
fn large_central_directory_size_uses_zip64() {
    let mut is_zip64 = false;

    let field = central_directory_size_field(NON_ZIP64_MAX_SIZE as u64 + 1, false, &mut is_zip64).unwrap();

    assert_eq!(field, NON_ZIP64_MAX_SIZE);
    assert!(is_zip64);
}

#[test]
fn large_central_directory_size_errors_without_zip64() {
    let mut is_zip64 = false;

    let result = central_directory_size_field(NON_ZIP64_MAX_SIZE as u64 + 1, true, &mut is_zip64);

    assert!(matches!(result, Err(ZipError::Zip64Needed(Zip64ErrorCase::LargeFile))));
    assert!(!is_zip64);
}
