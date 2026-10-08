// Copyright (c) 2021 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::error::{Result, ZipError};

#[cfg(any(
    feature = "deflate-write",
    feature = "bzip2-write",
    feature = "zstd-write",
    feature = "lzma-write",
    feature = "xz-write"
))]
use async_compression::Level;

/// A compression method supported by this crate.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Stored,
    #[cfg(any(feature = "deflate-read", feature = "deflate-write"))]
    Deflate,
    #[cfg(feature = "deflate64-read")]
    Deflate64,
    #[cfg(any(feature = "bzip2-read", feature = "bzip2-write"))]
    Bz,
    #[cfg(any(feature = "lzma-read", feature = "lzma-write"))]
    Lzma,
    #[cfg(any(feature = "zstd-read", feature = "zstd-write"))]
    Zstd,
    #[cfg(any(feature = "xz-read", feature = "xz-write"))]
    Xz,
}

impl Compression {
    /// Reject unsupported writers before emitting any entry data.
    pub(crate) fn ensure_can_write(self) -> Result<()> {
        match self {
            #[cfg(all(feature = "deflate-read", not(feature = "deflate-write")))]
            Self::Deflate => Err(ZipError::FeatureNotSupported("Deflate writing")),
            #[cfg(all(feature = "bzip2-read", not(feature = "bzip2-write")))]
            Self::Bz => Err(ZipError::FeatureNotSupported("Bz writing")),
            #[cfg(all(feature = "lzma-read", not(feature = "lzma-write")))]
            Self::Lzma => Err(ZipError::FeatureNotSupported("Lzma writing")),
            #[cfg(all(feature = "zstd-read", not(feature = "zstd-write")))]
            Self::Zstd => Err(ZipError::FeatureNotSupported("Zstd writing")),
            #[cfg(all(feature = "xz-read", not(feature = "xz-write")))]
            Self::Xz => Err(ZipError::FeatureNotSupported("Xz writing")),
            #[cfg(feature = "deflate64-read")]
            Self::Deflate64 => Err(ZipError::FeatureNotSupported("Deflate64 writing")),
            _ => Ok(()),
        }
    }
}

impl TryFrom<u16> for Compression {
    type Error = ZipError;

    // Convert a u16 stored with little endianness into a supported compression method.
    // https://github.com/Majored/rs-async-zip/blob/main/SPECIFICATION.md#445
    fn try_from(value: u16) -> Result<Self> {
        match value {
            0 => Ok(Compression::Stored),
            #[cfg(any(feature = "deflate-read", feature = "deflate-write"))]
            8 => Ok(Compression::Deflate),
            #[cfg(feature = "deflate64-read")]
            9 => Ok(Compression::Deflate64),
            #[cfg(any(feature = "bzip2-read", feature = "bzip2-write"))]
            12 => Ok(Compression::Bz),
            #[cfg(any(feature = "lzma-read", feature = "lzma-write"))]
            14 => Ok(Compression::Lzma),
            #[cfg(any(feature = "zstd-read", feature = "zstd-write"))]
            93 => Ok(Compression::Zstd),
            #[cfg(any(feature = "xz-read", feature = "xz-write"))]
            95 => Ok(Compression::Xz),
            _ => Err(ZipError::CompressionNotSupported(value)),
        }
    }
}

impl From<&Compression> for u16 {
    // Convert a supported compression method into its relevant u16 stored with little endianness.
    // https://github.com/Majored/rs-async-zip/blob/main/SPECIFICATION.md#445
    fn from(compression: &Compression) -> u16 {
        match compression {
            Compression::Stored => 0,
            #[cfg(any(feature = "deflate-read", feature = "deflate-write"))]
            Compression::Deflate => 8,
            #[cfg(feature = "deflate64-read")]
            Compression::Deflate64 => 9,
            #[cfg(any(feature = "bzip2-read", feature = "bzip2-write"))]
            Compression::Bz => 12,
            #[cfg(any(feature = "lzma-read", feature = "lzma-write"))]
            Compression::Lzma => 14,
            #[cfg(any(feature = "zstd-read", feature = "zstd-write"))]
            Compression::Zstd => 93,
            #[cfg(any(feature = "xz-read", feature = "xz-write"))]
            Compression::Xz => 95,
        }
    }
}

impl From<Compression> for u16 {
    fn from(compression: Compression) -> u16 {
        (&compression).into()
    }
}

/// Level of compression data should be compressed with for deflate.
#[derive(Debug, Clone, Copy)]
pub enum DeflateOption {
    // Normal (-en) compression option was used.
    Normal,

    // Maximum (-exx/-ex) compression option was used.
    Maximum,

    // Fast (-ef) compression option was used.
    Fast,

    // Super Fast (-es) compression option was used.
    Super,

    /// Other implementation defined level.
    Other(i32),
}

#[cfg(any(
    feature = "deflate-write",
    feature = "bzip2-write",
    feature = "zstd-write",
    feature = "lzma-write",
    feature = "xz-write"
))]
impl DeflateOption {
    pub(crate) fn into_level(self) -> Level {
        // FIXME: There's no clear documentation on what these specific levels defined in the ZIP specification relate
        // to. We want to be compatible with any other library, and not specific to `async_compression`'s levels.
        if let Self::Other(l) = self {
            Level::Precise(l)
        } else {
            Level::Default
        }
    }
}
