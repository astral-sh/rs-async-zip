use futures_lite::io::{AsyncRead, AsyncReadExt};

use crate::base::read::counting::Counting;
use crate::base::read::io::CombinedCentralDirectoryRecord;
use crate::base::read::{detect_filename, get_zip64_extra_field, io};
use crate::error::{Result, ZipError};
use crate::spec::consts::{CDH_SIGNATURE, EOCDR_SIGNATURE, NON_ZIP64_MAX_SIZE, ZIP64_EOCDR_SIGNATURE};
use crate::spec::header::{
    CentralDirectoryRecord, EndOfCentralDirectoryHeader, Zip64EndOfCentralDirectoryLocator,
    Zip64EndOfCentralDirectoryRecord,
};
use crate::spec::parse::parse_extra_fields;
use crate::ZipString;

/// An entry returned by the [`CentralDirectoryReader`].
pub enum Entry {
    CentralDirectoryEntry(CentralDirectoryEntry),
    EndOfCentralDirectoryRecord(CombinedCentralDirectoryRecord, ZipString),
}

/// An entry in the ZIP file's central directory.
pub struct CentralDirectoryEntry {
    /// The compressed size of the entry, taking into account ZIP64 if necessary.
    pub(crate) compressed_size: u64,
    /// The uncompressed size of the entry, taking into account ZIP64 if necessary.
    pub(crate) uncompressed_size: u64,
    /// The file offset of the entry in the ZIP file, taking into account ZIP64 if necessary.
    pub(crate) lh_offset: u64,
    /// The end-of-central-directory record header.
    pub(crate) header: CentralDirectoryRecord,
    /// The filename of the entry.
    pub(crate) filename: ZipString,
}

impl CentralDirectoryEntry {
    /// Returns the entry's filename.
    ///
    /// ## Note
    /// This will return the raw filename stored during ZIP creation. If calling this method on entries retrieved from
    /// untrusted ZIP files, the filename should be sanitised before being used as a path to prevent [directory
    /// traversal attacks](https://en.wikipedia.org/wiki/Directory_traversal_attack).
    pub fn filename(&self) -> &ZipString {
        &self.filename
    }

    /// Returns whether or not the entry represents a directory.
    pub fn dir(&self) -> Result<bool> {
        Ok(self.filename.as_str()?.ends_with('/'))
    }

    /// Returns the entry's integer-based UNIX permissions.
    pub fn unix_permissions(&self) -> Option<u32> {
        Some((self.header.exter_attr) >> 16)
    }

    /// Returns the CRC32 checksum of the entry.
    pub fn crc32(&self) -> u32 {
        self.header.crc
    }

    /// Returns the file offset of the entry in the ZIP file.
    pub fn file_offset(&self) -> u64 {
        self.lh_offset
    }

    /// Returns the entry's compressed size.
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Returns the entry's uncompressed size.
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }
}

#[derive(Clone)]
pub struct CentralDirectoryReader<R> {
    reader: R,
    initial: bool,
    offset: u64,
}

impl<'a, R> CentralDirectoryReader<Counting<R>>
where
    R: AsyncRead + Unpin + 'a,
{
    /// Constructs a new ZIP reader from a non-seekable source.
    pub fn new(reader: R, offset: u64) -> Self {
        Self { reader: Counting::new(reader), offset, initial: true }
    }

    /// Reads the next [`CentralDirectoryEntry`] from the underlying source, advancing the
    /// reader to the next record.
    ///
    /// Returns `Ok(EndOfCentralDirectoryRecord)` if the end of the central directory record has
    /// been reached.
    pub async fn next(&mut self) -> Result<Entry> {
        // Skip the first `CDH_SIGNATURE`. The `CentralDirectoryReader` is assumed to pick up from
        // where the streaming `ZipFileReader` left off, which means that the first record's
        // signature has already been read.
        if self.initial {
            self.initial = false;
        } else {
            let signature = {
                let mut buffer = [0; 4];
                self.reader.read_exact(&mut buffer).await?;
                u32::from_le_bytes(buffer)
            };
            let offset = self.offset + self.reader.bytes_read();
            match signature {
                CDH_SIGNATURE => (),
                EOCDR_SIGNATURE => {
                    // Read the end-of-central-directory header.
                    let eocdr = EndOfCentralDirectoryHeader::from_reader(&mut self.reader).await?;

                    // Read the EOCDR comment.
                    let comment =
                        io::read_string(&mut self.reader, eocdr.file_comm_length.into(), crate::StringEncoding::Utf8)
                            .await?;

                    return Ok(Entry::EndOfCentralDirectoryRecord(
                        CombinedCentralDirectoryRecord::from(&eocdr),
                        comment,
                    ));
                }
                ZIP64_EOCDR_SIGNATURE => {
                    // Read the ZIP64 EOCDR.
                    let zip64_eocdr = Zip64EndOfCentralDirectoryRecord::from_reader(&mut self.reader).await?;

                    // Read the ZIP64 EOCDR locator.
                    let Some(zip64_eocdl) =
                        Zip64EndOfCentralDirectoryLocator::try_from_reader(&mut self.reader).await?
                    else {
                        return Err(ZipError::MissingZip64EndOfCentralDirectoryLocator);
                    };

                    // Verify that the ZIP64 EOCDR locator points to the correct offset.
                    if zip64_eocdl.relative_offset != offset {
                        return Err(ZipError::InvalidZip64EndOfCentralDirectoryLocatorOffset(
                            zip64_eocdl.relative_offset,
                            offset,
                        ));
                    }

                    // Read the EOCDR signature.
                    let signature = {
                        let mut buffer = [0; 4];
                        self.reader.read_exact(&mut buffer).await?;
                        u32::from_le_bytes(buffer)
                    };
                    if signature != EOCDR_SIGNATURE {
                        return Err(ZipError::UnexpectedHeaderError(signature, EOCDR_SIGNATURE));
                    }

                    // Read the end-of-central-directory header.
                    let eocdr = EndOfCentralDirectoryHeader::from_reader(&mut self.reader).await?;

                    // Read the EOCDR comment.
                    let comment =
                        io::read_string(&mut self.reader, eocdr.file_comm_length.into(), crate::StringEncoding::Utf8)
                            .await?;

                    return Ok(Entry::EndOfCentralDirectoryRecord(
                        CombinedCentralDirectoryRecord::combine(eocdr, zip64_eocdr),
                        comment,
                    ));
                }
                actual => return Err(ZipError::UnexpectedHeaderError(actual, CDH_SIGNATURE)),
            }
        }

        // Read the record.
        let header = CentralDirectoryRecord::from_reader(&mut self.reader).await?;

        // Read the file name and extra field, which also ensures that we advance the reader to the
        // next record.
        let filename_basic = io::read_bytes(&mut self.reader, header.file_name_length.into()).await?;
        let extra_field = io::read_bytes(&mut self.reader, header.extra_field_length.into()).await?;
        let extra_fields = parse_extra_fields(
            extra_field,
            header.uncompressed_size,
            header.compressed_size,
            Some(header.lh_offset),
            Some(header.disk_start),
        )?;
        let zip64_extra_field = get_zip64_extra_field(&extra_fields);

        // Reconcile the compressed size, uncompressed size, and file offset, using ZIP64 if necessary.
        let compressed_size = if let Some(compressed_size) = zip64_extra_field
            .and_then(|zip64| zip64.compressed_size)
            .filter(|_| header.compressed_size == NON_ZIP64_MAX_SIZE)
        {
            compressed_size
        } else {
            header.compressed_size as u64
        };
        let uncompressed_size = if let Some(uncompressed_size) = zip64_extra_field
            .and_then(|zip64| zip64.uncompressed_size)
            .filter(|_| header.uncompressed_size == NON_ZIP64_MAX_SIZE)
        {
            uncompressed_size
        } else {
            header.uncompressed_size as u64
        };
        let lh_offset = if let Some(lh_offset) = zip64_extra_field
            .and_then(|zip64| zip64.relative_header_offset)
            .filter(|_| header.lh_offset == NON_ZIP64_MAX_SIZE)
        {
            lh_offset
        } else {
            header.lh_offset as u64
        };

        // Parse out the filename.
        let filename = detect_filename(filename_basic, header.flags.filename_unicode, extra_fields.as_ref());

        Ok(Entry::CentralDirectoryEntry(CentralDirectoryEntry {
            header,
            compressed_size,
            uncompressed_size,
            lh_offset,
            filename,
        }))
    }
}
