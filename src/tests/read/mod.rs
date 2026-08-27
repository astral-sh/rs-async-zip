// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

pub(crate) mod cd;
pub(crate) mod compression;
pub(crate) mod encryption;
pub(crate) mod locator;
pub(crate) mod stream;
pub(crate) mod version;
pub(crate) mod zip64;

use std::{
    io::{Result, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use futures_lite::io::{AsyncRead, AsyncSeek};

use crate::error::ZipError;

struct ShortReader<R> {
    inner: R,
    max_read: usize,
    eof_error: Option<std::io::ErrorKind>,
    bytes_read: usize,
    seeks: usize,
}

impl<R> ShortReader<R> {
    fn new(inner: R, max_read: usize) -> Self {
        Self { inner, max_read, eof_error: None, bytes_read: 0, seeks: 0 }
    }

    fn with_eof_error(mut self, kind: std::io::ErrorKind) -> Self {
        self.eof_error = Some(kind);
        self
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ShortReader<R> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let max_read = self.max_read.min(buf.len());
        match Pin::new(&mut self.inner).poll_read(cx, &mut buf[..max_read]) {
            Poll::Ready(Ok(0)) if !buf.is_empty() && self.eof_error.is_some() => {
                Poll::Ready(Err(std::io::Error::new(self.eof_error.unwrap(), "suffix read failed")))
            }
            Poll::Ready(Ok(read)) => {
                self.bytes_read += read;
                Poll::Ready(Ok(read))
            }
            result => result,
        }
    }
}

impl<R: AsyncSeek + Unpin> AsyncSeek for ShortReader<R> {
    fn poll_seek(mut self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<Result<u64>> {
        let result = Pin::new(&mut self.inner).poll_seek(cx, pos);
        if result.is_ready() {
            self.seeks += 1;
        }
        result
    }
}

fn assert_unexpected_eof(error: ZipError) {
    let ZipError::UpstreamReadError(error) = error else {
        panic!("expected an upstream read error, got {error:?}");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn test_truncated_eocdr_is_rejected() {
    use futures_lite::io::Cursor;

    use crate::base::read::seek::ZipFileReader;

    // Neither declared comment bytes nor zero-filled EOCD fields are optional padding.
    let empty = include_bytes!("locator/empty.zip");
    for data in [include_bytes!("truncated/empty-with-max-comment.zip").as_slice(), &empty[..empty.len() - 1]] {
        let Err(error) = ZipFileReader::new(Cursor::new(data)).await else {
            panic!("expected a truncated EOCD to fail");
        };
        assert_unexpected_eof(error);
    }
}

#[tokio::test]
async fn test_truncated_local_filename_is_rejected() {
    use crate::base::read::stream::ZipFileReader;

    // The fixture ends one byte before its declared local filename length.
    let reader = ZipFileReader::new(include_bytes!("truncated/diff-004-sample.zip").as_slice());
    let Err(error) = reader.next_with_entry().await else {
        panic!("expected a truncated local filename to fail");
    };

    assert_unexpected_eof(error);
}

#[tokio::test]
async fn test_truncated_local_extra_field_is_rejected() {
    use crate::base::read::stream::ZipFileReader;

    // The fixture ends one byte before its declared local extra-field length.
    let reader = ZipFileReader::new(include_bytes!("truncated/diff-002-sample.zip").as_slice());
    let Err(error) = reader.next_with_entry().await else {
        panic!("expected a truncated local extra field to fail");
    };

    assert_unexpected_eof(error);
}

#[tokio::test]
async fn test_truncated_central_directory_comment_is_rejected() {
    use futures_lite::io::Cursor;

    use crate::base::read::{cd::CentralDirectoryReader, stream::ZipFileReader};

    // The fixture ends one byte before its declared central-directory comment length.
    let mut cursor = Cursor::new(include_bytes!("truncated/diff-094-sample.zip"));
    let mut zip = ZipFileReader::new(&mut cursor);
    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await.unwrap() {
        (.., zip) = entry.skip().await.unwrap();
        offset = zip.offset();
    }

    let mut reader = CentralDirectoryReader::new(&mut cursor, offset);
    let Err(error) = reader.next().await else {
        panic!("expected a truncated central-directory comment to fail");
    };

    assert_unexpected_eof(error);
}
