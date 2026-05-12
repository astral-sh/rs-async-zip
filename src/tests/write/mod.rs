// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use futures_lite::io::{AsyncWrite, AsyncWriteExt};
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::base::write::{central_directory_size_field, ZipFileWriter};
use crate::error::{Zip64ErrorCase, ZipError};
use crate::spec::consts::{CDH_SIGNATURE, LFH_SIGNATURE, NON_ZIP64_MAX_SIZE};
use crate::{Compression, ZipEntryBuilder};

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

fn assert_default_modification_date(buffer: &[u8]) {
    let local_header_signature = LFH_SIGNATURE.to_le_bytes();
    assert_eq!(&buffer[..local_header_signature.len()], local_header_signature);
    assert_eq!(u16::from_le_bytes(buffer[10..12].try_into().unwrap()), 0);
    assert_eq!(u16::from_le_bytes(buffer[12..14].try_into().unwrap()), 0x21);

    let central_directory_signature = CDH_SIGNATURE.to_le_bytes();
    let central_directory_offset = buffer
        .windows(central_directory_signature.len())
        .position(|window| window == central_directory_signature)
        .unwrap();
    assert_eq!(
        u16::from_le_bytes(buffer[central_directory_offset + 12..central_directory_offset + 14].try_into().unwrap()),
        0
    );
    assert_eq!(
        u16::from_le_bytes(buffer[central_directory_offset + 14..central_directory_offset + 16].try_into().unwrap()),
        0x21
    );
}

#[tokio::test]
async fn default_modification_date_is_valid_for_whole_writes() {
    let mut buffer = Vec::new();
    let mut writer = ZipFileWriter::new(&mut buffer);
    let entry = ZipEntryBuilder::new("file".into(), Compression::Stored);

    writer.write_entry_whole(entry, b"data").await.unwrap();
    writer.close().await.unwrap();

    assert_default_modification_date(&buffer);
}

#[tokio::test]
async fn default_modification_date_is_valid_for_stream_writes() {
    let mut buffer = Vec::new();
    let mut writer = ZipFileWriter::new(&mut buffer);
    let entry = ZipEntryBuilder::new("file".into(), Compression::Stored);

    let mut entry_writer = writer.write_entry_stream(entry).await.unwrap();
    entry_writer.write_all(b"data").await.unwrap();
    entry_writer.close().await.unwrap();
    writer.close().await.unwrap();

    assert_default_modification_date(&buffer);
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

#[cfg(feature = "deflate64")]
#[tokio::test]
async fn reject_deflate64_whole_writes() {
    let mut buffer = Vec::new();
    let mut writer = ZipFileWriter::new(&mut buffer);
    let entry = ZipEntryBuilder::new("file".into(), Compression::Deflate64);

    let result = writer.write_entry_whole(entry, b"data").await;

    assert!(matches!(result, Err(ZipError::FeatureNotSupported("Deflate64 writing"))));
    assert!(buffer.is_empty());
}

#[cfg(feature = "deflate64")]
#[tokio::test]
async fn reject_deflate64_stream_writes() {
    let mut buffer = Vec::new();
    let mut writer = ZipFileWriter::new(&mut buffer);
    let entry = ZipEntryBuilder::new("file".into(), Compression::Deflate64);

    let result = writer.write_entry_stream(entry).await;

    assert!(matches!(result, Err(ZipError::FeatureNotSupported("Deflate64 writing"))));
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
