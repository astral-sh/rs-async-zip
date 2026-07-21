// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_lite::io::{AsyncRead, AsyncSeek, Cursor, SeekFrom};

struct ShortReadCursor {
    inner: Cursor<Vec<u8>>,
    max_read: usize,
}

impl AsyncRead for ShortReadCursor {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let max_read = self.max_read.min(buf.len());
        Pin::new(&mut self.inner).poll_read(cx, &mut buf[..max_read])
    }
}

impl AsyncSeek for ShortReadCursor {
    fn poll_seek(mut self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<Result<u64>> {
        Pin::new(&mut self.inner).poll_seek(cx, pos)
    }
}

#[test]
fn search_one_byte_test() {
    let buffer: &[u8] = &[0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_none());

    let buffer: &[u8] = &[0x2, 0x1, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_some());
    assert_eq!(1, matched.unwrap());
}

#[test]
fn search_two_byte_test() {
    let buffer: &[u8] = &[0x2, 0x1, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x2, 0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_some());
    assert_eq!(1, matched.unwrap());
}

#[tokio::test]
async fn locator_empty_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_empty_max_comment_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty-with-max-comment.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_buffer_boundary_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty-buffer-boundary.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_handles_short_reads_without_skipping_bytes() {
    let signature = crate::spec::consts::EOCDR_SIGNATURE.to_le_bytes();
    let signature_offset = 3_000;
    let mut data = vec![0; 4_096];
    data[signature_offset..signature_offset + signature.len()].copy_from_slice(&signature);

    let mut reader = ShortReadCursor { inner: Cursor::new(data), max_read: 64 };
    let eocdr = crate::base::read::io::locator::eocdr(&mut reader).await.unwrap();

    assert_eq!(eocdr, (signature_offset + signature.len()) as u64);
}
