// Copyright (c) 2021 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::base::write::io::offset::AsyncOffsetWriter;
use crate::error::Result;
use crate::spec::Compression;

use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(any(
    feature = "deflate-write",
    feature = "bzip2-write",
    feature = "zstd-write",
    feature = "lzma-write",
    feature = "xz-write"
))]
use async_compression::futures::write;
use futures_lite::io::AsyncWrite;

pub enum CompressedAsyncWriter<'b, W: AsyncWrite + Unpin> {
    Stored(ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>),
    #[cfg(feature = "deflate-write")]
    Deflate(write::DeflateEncoder<ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>>),
    #[cfg(feature = "bzip2-write")]
    Bz(write::BzEncoder<ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>>),
    #[cfg(feature = "lzma-write")]
    Lzma(write::LzmaEncoder<ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>>),
    #[cfg(feature = "zstd-write")]
    Zstd(write::ZstdEncoder<ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>>),
    #[cfg(feature = "xz-write")]
    Xz(write::XzEncoder<ShutdownIgnoredWriter<&'b mut AsyncOffsetWriter<W>>>),
}

impl<'b, W: AsyncWrite + Unpin> CompressedAsyncWriter<'b, W> {
    pub fn from_raw(writer: &'b mut AsyncOffsetWriter<W>, compression: Compression) -> Result<Self> {
        compression.ensure_can_write()?;

        Ok(match compression {
            Compression::Stored => CompressedAsyncWriter::Stored(ShutdownIgnoredWriter(writer)),
            #[cfg(feature = "deflate-write")]
            Compression::Deflate => {
                CompressedAsyncWriter::Deflate(write::DeflateEncoder::new(ShutdownIgnoredWriter(writer)))
            }
            #[cfg(feature = "bzip2-write")]
            Compression::Bz => CompressedAsyncWriter::Bz(write::BzEncoder::new(ShutdownIgnoredWriter(writer))),
            #[cfg(feature = "lzma-write")]
            Compression::Lzma => CompressedAsyncWriter::Lzma(write::LzmaEncoder::new(ShutdownIgnoredWriter(writer))),
            #[cfg(feature = "zstd-write")]
            Compression::Zstd => CompressedAsyncWriter::Zstd(write::ZstdEncoder::new(ShutdownIgnoredWriter(writer))),
            #[cfg(feature = "xz-write")]
            Compression::Xz => CompressedAsyncWriter::Xz(write::XzEncoder::new(ShutdownIgnoredWriter(writer))),
            #[allow(unreachable_patterns)]
            _ => unreachable!("write support was checked above"),
        })
    }

    pub fn into_inner(self) -> &'b mut AsyncOffsetWriter<W> {
        match self {
            CompressedAsyncWriter::Stored(inner) => inner.into_inner(),
            #[cfg(feature = "deflate-write")]
            CompressedAsyncWriter::Deflate(inner) => inner.into_inner().into_inner(),
            #[cfg(feature = "bzip2-write")]
            CompressedAsyncWriter::Bz(inner) => inner.into_inner().into_inner(),
            #[cfg(feature = "lzma-write")]
            CompressedAsyncWriter::Lzma(inner) => inner.into_inner().into_inner(),
            #[cfg(feature = "zstd-write")]
            CompressedAsyncWriter::Zstd(inner) => inner.into_inner().into_inner(),
            #[cfg(feature = "xz-write")]
            CompressedAsyncWriter::Xz(inner) => inner.into_inner().into_inner(),
        }
    }
}

impl<'b, W: AsyncWrite + Unpin> AsyncWrite for CompressedAsyncWriter<'b, W> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<std::result::Result<usize, Error>> {
        match *self {
            CompressedAsyncWriter::Stored(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(feature = "deflate-write")]
            CompressedAsyncWriter::Deflate(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(feature = "bzip2-write")]
            CompressedAsyncWriter::Bz(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(feature = "lzma-write")]
            CompressedAsyncWriter::Lzma(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(feature = "zstd-write")]
            CompressedAsyncWriter::Zstd(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(feature = "xz-write")]
            CompressedAsyncWriter::Xz(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::result::Result<(), Error>> {
        match *self {
            CompressedAsyncWriter::Stored(ref mut inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(feature = "deflate-write")]
            CompressedAsyncWriter::Deflate(ref mut inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(feature = "bzip2-write")]
            CompressedAsyncWriter::Bz(ref mut inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(feature = "lzma-write")]
            CompressedAsyncWriter::Lzma(ref mut inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(feature = "zstd-write")]
            CompressedAsyncWriter::Zstd(ref mut inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(feature = "xz-write")]
            CompressedAsyncWriter::Xz(ref mut inner) => Pin::new(inner).poll_flush(cx),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::result::Result<(), Error>> {
        match *self {
            CompressedAsyncWriter::Stored(ref mut inner) => Pin::new(inner).poll_close(cx),
            #[cfg(feature = "deflate-write")]
            CompressedAsyncWriter::Deflate(ref mut inner) => Pin::new(inner).poll_close(cx),
            #[cfg(feature = "bzip2-write")]
            CompressedAsyncWriter::Bz(ref mut inner) => Pin::new(inner).poll_close(cx),
            #[cfg(feature = "lzma-write")]
            CompressedAsyncWriter::Lzma(ref mut inner) => Pin::new(inner).poll_close(cx),
            #[cfg(feature = "zstd-write")]
            CompressedAsyncWriter::Zstd(ref mut inner) => Pin::new(inner).poll_close(cx),
            #[cfg(feature = "xz-write")]
            CompressedAsyncWriter::Xz(ref mut inner) => Pin::new(inner).poll_close(cx),
        }
    }
}

pub struct ShutdownIgnoredWriter<W: AsyncWrite + Unpin>(W);

impl<W: AsyncWrite + Unpin> ShutdownIgnoredWriter<W> {
    pub fn into_inner(self) -> W {
        self.0
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ShutdownIgnoredWriter<W> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<std::result::Result<usize, Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::result::Result<(), Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context) -> Poll<std::result::Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}
