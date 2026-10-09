// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::error::{Result, ZipError};
use crate::spec::Compression;

use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(any(
    feature = "deflate-read",
    feature = "bzip2-read",
    feature = "zstd-read",
    feature = "lzma-read",
    feature = "xz-read",
    feature = "deflate64-read"
))]
use async_compression::futures::bufread;
use futures_lite::io::{AsyncBufRead, AsyncRead};
use pin_project::pin_project;

/// A wrapping reader which holds concrete types for all respective compression method readers.
#[pin_project(project = CompressedReaderProj)]
pub(crate) enum CompressedReader<R> {
    Stored(#[pin] R),
    #[cfg(feature = "deflate-read")]
    Deflate(#[pin] bufread::DeflateDecoder<R>),
    #[cfg(feature = "deflate64-read")]
    Deflate64(#[pin] bufread::Deflate64Decoder<R>),
    #[cfg(feature = "bzip2-read")]
    Bz(#[pin] bufread::BzDecoder<R>),
    #[cfg(feature = "lzma-read")]
    Lzma(#[pin] bufread::LzmaDecoder<R>),
    #[cfg(feature = "zstd-read")]
    Zstd(#[pin] bufread::ZstdDecoder<R>),
    #[cfg(feature = "xz-read")]
    Xz(#[pin] bufread::XzDecoder<R>),
}

impl<R> CompressedReader<R>
where
    R: AsyncBufRead + Unpin,
{
    /// Constructs a new wrapping reader from a generic [`AsyncBufRead`] implementer.
    pub(crate) fn new(reader: R, compression: Compression) -> Result<Self> {
        Ok(match compression {
            Compression::Stored => CompressedReader::Stored(reader),
            #[cfg(feature = "deflate-read")]
            Compression::Deflate => CompressedReader::Deflate(bufread::DeflateDecoder::new(reader)),
            #[cfg(feature = "deflate64-read")]
            Compression::Deflate64 => CompressedReader::Deflate64(bufread::Deflate64Decoder::new(reader)),
            #[cfg(feature = "bzip2-read")]
            Compression::Bz => CompressedReader::Bz(bufread::BzDecoder::new(reader)),
            #[cfg(feature = "lzma-read")]
            Compression::Lzma => CompressedReader::Lzma(bufread::LzmaDecoder::new(reader)),
            #[cfg(feature = "zstd-read")]
            Compression::Zstd => CompressedReader::Zstd(bufread::ZstdDecoder::new(reader)),
            #[cfg(feature = "xz-read")]
            Compression::Xz => CompressedReader::Xz(bufread::XzDecoder::new(reader)),
            #[allow(unreachable_patterns)]
            _ => return Err(ZipError::CompressionNotSupported(compression.into())),
        })
    }

    /// Consumes this reader and returns the inner value.
    pub(crate) fn inner(&self) -> &R {
        match self {
            CompressedReader::Stored(inner) => inner,
            #[cfg(feature = "deflate-read")]
            CompressedReader::Deflate(inner) => inner.get_ref(),
            #[cfg(feature = "deflate64-read")]
            CompressedReader::Deflate64(inner) => inner.get_ref(),
            #[cfg(feature = "bzip2-read")]
            CompressedReader::Bz(inner) => inner.get_ref(),
            #[cfg(feature = "lzma-read")]
            CompressedReader::Lzma(inner) => inner.get_ref(),
            #[cfg(feature = "zstd-read")]
            CompressedReader::Zstd(inner) => inner.get_ref(),
            #[cfg(feature = "xz-read")]
            CompressedReader::Xz(inner) => inner.get_ref(),
        }
    }

    /// Consumes this reader and returns the inner value.
    pub(crate) fn into_inner(self) -> R {
        match self {
            CompressedReader::Stored(inner) => inner,
            #[cfg(feature = "deflate-read")]
            CompressedReader::Deflate(inner) => inner.into_inner(),
            #[cfg(feature = "deflate64-read")]
            CompressedReader::Deflate64(inner) => inner.into_inner(),
            #[cfg(feature = "bzip2-read")]
            CompressedReader::Bz(inner) => inner.into_inner(),
            #[cfg(feature = "lzma-read")]
            CompressedReader::Lzma(inner) => inner.into_inner(),
            #[cfg(feature = "zstd-read")]
            CompressedReader::Zstd(inner) => inner.into_inner(),
            #[cfg(feature = "xz-read")]
            CompressedReader::Xz(inner) => inner.into_inner(),
        }
    }
}

impl<R> AsyncRead for CompressedReader<R>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_read(self: Pin<&mut Self>, c: &mut Context<'_>, b: &mut [u8]) -> Poll<std::io::Result<usize>> {
        match self.project() {
            CompressedReaderProj::Stored(inner) => inner.poll_read(c, b),
            #[cfg(feature = "deflate-read")]
            CompressedReaderProj::Deflate(inner) => inner.poll_read(c, b),
            #[cfg(feature = "deflate64-read")]
            CompressedReaderProj::Deflate64(inner) => inner.poll_read(c, b),
            #[cfg(feature = "bzip2-read")]
            CompressedReaderProj::Bz(inner) => inner.poll_read(c, b),
            #[cfg(feature = "lzma-read")]
            CompressedReaderProj::Lzma(inner) => inner.poll_read(c, b),
            #[cfg(feature = "zstd-read")]
            CompressedReaderProj::Zstd(inner) => inner.poll_read(c, b),
            #[cfg(feature = "xz-read")]
            CompressedReaderProj::Xz(inner) => inner.poll_read(c, b),
        }
    }
}
