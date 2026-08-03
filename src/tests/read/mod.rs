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

struct ShortReader<R> {
    inner: R,
    max_read: usize,
}

impl<R> ShortReader<R> {
    fn new(inner: R, max_read: usize) -> Self {
        Self { inner, max_read }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ShortReader<R> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let max_read = self.max_read.min(buf.len());
        Pin::new(&mut self.inner).poll_read(cx, &mut buf[..max_read])
    }
}

impl<R: AsyncSeek + Unpin> AsyncSeek for ShortReader<R> {
    fn poll_seek(mut self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<Result<u64>> {
        Pin::new(&mut self.inner).poll_seek(cx, pos)
    }
}
