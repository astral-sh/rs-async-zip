// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::base::read::counting::Counting;
use crate::base::read::io::{compressed::CompressedReader, hashed::HashedReader, owned::OwnedReader};
use crate::entry::ZipEntry;
use crate::error::{Result, ZipError};
use crate::spec::consts::{
    DATA_DESCRIPTOR_LENGTH, DATA_DESCRIPTOR_SIGNATURE, SIGNATURE_LENGTH, ZIP64_DATA_DESCRIPTOR_LENGTH,
};
use crate::spec::data_descriptor::{DataDescriptor, Zip64DataDescriptor};
use crate::spec::Compression;

use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_lite::io::{AsyncBufRead, AsyncRead, AsyncReadExt, Take};
use pin_project::pin_project;

/// A type which encodes that [`ZipEntryReader`] has associated entry data.
pub struct WithEntry<'a>(OwnedEntry<'a>);

/// A type which encodes that [`ZipEntryReader`] has no associated entry data.
pub struct WithoutEntry;

/// Expected input length and descriptor for an entry opened from a central directory.
pub(crate) struct EntryValidation {
    compressed_size: u64,
    descriptor: [u8; SIGNATURE_LENGTH + ZIP64_DATA_DESCRIPTOR_LENGTH],
    descriptor_read: usize,
    descriptor_end: usize,
    data_finished: bool,
}

impl EntryValidation {
    /// The local record must end exactly at its next indexed boundary. That leaves either
    /// no suffix, or one descriptor of the local header's width, with an optional signature.
    pub(crate) fn new(entry: &ZipEntry, zip64: bool, suffix_length: u64) -> Option<Self> {
        let mut validation = Self {
            compressed_size: entry.compressed_size(),
            descriptor: [0; SIGNATURE_LENGTH + ZIP64_DATA_DESCRIPTOR_LENGTH],
            descriptor_read: 0,
            descriptor_end: 0,
            data_finished: false,
        };
        if !entry.data_descriptor() {
            return (suffix_length == 0).then_some(validation);
        }

        // Compare the wire representation directly at EOF. The known record boundary removes
        // the ambiguity of an unsigned descriptor whose CRC equals the optional signature.
        validation.descriptor[..SIGNATURE_LENGTH].copy_from_slice(&DATA_DESCRIPTOR_SIGNATURE.to_le_bytes());
        let length = if zip64 {
            let descriptor = Zip64DataDescriptor {
                crc: entry.crc32(),
                compressed_size: entry.compressed_size(),
                uncompressed_size: entry.uncompressed_size(),
            };
            validation.descriptor[SIGNATURE_LENGTH..].copy_from_slice(&descriptor.as_bytes());
            ZIP64_DATA_DESCRIPTOR_LENGTH
        } else {
            let descriptor = DataDescriptor {
                crc: entry.crc32(),
                compressed_size: entry.compressed_size().try_into().ok()?,
                uncompressed_size: entry.uncompressed_size().try_into().ok()?,
            };
            validation.descriptor[SIGNATURE_LENGTH..SIGNATURE_LENGTH + DATA_DESCRIPTOR_LENGTH]
                .copy_from_slice(&descriptor.as_bytes());
            DATA_DESCRIPTOR_LENGTH
        };
        validation.descriptor_read = match suffix_length {
            n if n == length as u64 => SIGNATURE_LENGTH,
            n if n == (length + SIGNATURE_LENGTH) as u64 => 0,
            _ => return None,
        };
        validation.descriptor_end = SIGNATURE_LENGTH + length;
        Some(validation)
    }
}

/// A ZIP entry reader which may implement decompression.
///
/// Entries opened through seek, memory, or filesystem readers validate their compressed length
/// and data descriptor before a nonempty read reports EOF. Read every entry to EOF, including
/// directories, to validate all archive boundaries. Dropping a partial reader does not validate its
/// unread contents. CRC checks remain available through the checked read helpers.
#[pin_project]
pub struct ZipEntryReader<'a, R, E> {
    #[pin]
    reader: HashedReader<CompressedReader<Counting<Take<OwnedReader<'a, R>>>>>,
    entry: E,
    validation: Option<EntryValidation>,
}

impl<'a, R> ZipEntryReader<'a, R, WithoutEntry>
where
    R: AsyncBufRead + Unpin,
{
    /// Constructs a new entry reader from its required parameters (incl. an owned R).
    pub(crate) fn new_with_owned(reader: R, compression: Compression, size: u64) -> Self {
        let reader =
            HashedReader::new(CompressedReader::new(Counting::new(OwnedReader::Owned(reader).take(size)), compression));
        Self { reader, entry: WithoutEntry, validation: None }
    }

    /// Constructs a new entry reader from its required parameters (incl. a mutable borrow of an R).
    pub(crate) fn new_with_borrow(reader: &'a mut R, compression: Compression, size: u64) -> Self {
        let reader = HashedReader::new(CompressedReader::new(
            Counting::new(OwnedReader::Borrow(reader).take(size)),
            compression,
        ));
        Self { reader, entry: WithoutEntry, validation: None }
    }

    pub(crate) fn with_validation(mut self, validation: EntryValidation) -> Self {
        self.validation = Some(validation);
        self
    }

    pub(crate) fn into_with_entry(self, entry: &'a ZipEntry) -> ZipEntryReader<'a, R, WithEntry<'a>> {
        ZipEntryReader { reader: self.reader, entry: WithEntry(OwnedEntry::Borrow(entry)), validation: self.validation }
    }

    pub(crate) fn into_with_entry_owned(self, entry: ZipEntry) -> ZipEntryReader<'a, R, WithEntry<'a>> {
        ZipEntryReader { reader: self.reader, entry: WithEntry(OwnedEntry::Owned(entry)), validation: self.validation }
    }
}

impl<'a, R, E> AsyncRead for ZipEntryReader<'a, R, E>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_read(self: Pin<&mut Self>, c: &mut Context<'_>, b: &mut [u8]) -> Poll<std::io::Result<usize>> {
        if b.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let this = self.project();
        let reader = this.reader.get_mut();
        if !this.validation.as_ref().is_some_and(|validation| validation.data_finished) {
            let read = ready!(Pin::new(&mut *reader).poll_read(c, b))?;
            if read != 0 {
                return Poll::Ready(Ok(read));
            }
        }
        let Some(validation) = this.validation else {
            return Poll::Ready(Ok(0));
        };
        validation.data_finished = true;
        let actual = reader.inner().inner().bytes_read();
        if actual != validation.compressed_size {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ZipError::CompressedSizeMismatch { expected: validation.compressed_size, actual },
            )));
        }

        // Bypass the exhausted compressed-data limit and counter. Descriptor bytes are not
        // payload, and the decoder must not run again after EOF, including across Pending.
        let source = reader.reader.inner_mut().inner_mut().get_mut();
        while validation.descriptor_read < validation.descriptor_end {
            let buffer = ready!(Pin::new(&mut *source).poll_fill_buf(c))?;
            let count = buffer.len().min(validation.descriptor_end - validation.descriptor_read);
            if count == 0 {
                return Poll::Ready(Err(std::io::ErrorKind::UnexpectedEof.into()));
            }
            if buffer[..count] != validation.descriptor[validation.descriptor_read..validation.descriptor_read + count]
            {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    ZipError::DataDescriptorMismatch,
                )));
            }
            Pin::new(&mut *source).consume(count);
            validation.descriptor_read += count;
        }
        Poll::Ready(Ok(0))
    }
}

impl<'a, R, E> ZipEntryReader<'a, R, E>
where
    R: AsyncBufRead + Unpin,
{
    /// Computes and returns the CRC32 hash of bytes read by this reader so far.
    ///
    /// This hash should only be computed once EOF has been reached.
    pub fn compute_hash(&mut self) -> u32 {
        self.reader.swap_and_compute_hash()
    }

    /// Return the number of bytes read so far by this reader.
    pub fn bytes_read(&self) -> u64 {
        self.reader.inner().inner().bytes_read()
    }

    /// Consumes this reader and returns the inner value.
    pub(crate) fn into_inner(self) -> R {
        self.reader.into_inner().into_inner().into_inner().into_inner().owned_into_inner()
    }
}

impl<R> ZipEntryReader<'_, R, WithEntry<'_>>
where
    R: AsyncBufRead + Unpin,
{
    /// Returns an immutable reference to the associated entry data.
    pub fn entry(&self) -> &'_ ZipEntry {
        self.entry.0.entry()
    }

    /// Reads all bytes until EOF has been reached, appending them to buf, and verifies the CRC32 values.
    ///
    /// This is a helper function synonymous to [`AsyncReadExt::read_to_end()`].
    pub async fn read_to_end_checked(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        let read = self.read_to_end(buf).await?;

        if self.compute_hash() == self.entry.0.entry().crc32() {
            Ok(read)
        } else {
            Err(ZipError::CRC32CheckError)
        }
    }

    /// Reads all bytes until EOF has been reached, placing them into buf, and verifies the CRC32 values.
    ///
    /// This is a helper function synonymous to [`AsyncReadExt::read_to_string()`].
    pub async fn read_to_string_checked(&mut self, buf: &mut String) -> Result<usize> {
        let read = self.read_to_string(buf).await?;

        if self.compute_hash() == self.entry.0.entry().crc32() {
            Ok(read)
        } else {
            Err(ZipError::CRC32CheckError)
        }
    }
}

enum OwnedEntry<'a> {
    Owned(ZipEntry),
    Borrow(&'a ZipEntry),
}

impl<'a> OwnedEntry<'a> {
    pub fn entry(&self) -> &'_ ZipEntry {
        match self {
            OwnedEntry::Owned(entry) => entry,
            OwnedEntry::Borrow(entry) => entry,
        }
    }
}
