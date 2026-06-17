// Copyright (c) 2025 Astral
// MIT License (https://github.com/astral-sh/rs-async-zip/blob/main/LICENSE)

use crate::spec::version::MAX_SUPPORTED_EXTRACT_VERSION;

const UNSUPPORTED_EXTRACT_VERSION: u16 = MAX_SUPPORTED_EXTRACT_VERSION + 1;

fn diff_092_data(local_version: u16, central_version: u16) -> Vec<u8> {
    use crate::spec::consts::CDH_SIGNATURE;

    let mut data = include_bytes!("diff-092-sample.zip").to_vec();
    data[4..6].copy_from_slice(&local_version.to_le_bytes());

    let signature = CDH_SIGNATURE.to_le_bytes();
    let offset = data.windows(signature.len()).position(|window| window == signature).unwrap();
    data[offset + 6..offset + 8].copy_from_slice(&central_version.to_le_bytes());
    data
}

fn central_directory_offset(data: &[u8]) -> usize {
    use crate::spec::consts::CDH_SIGNATURE;

    let signature = CDH_SIGNATURE.to_le_bytes();
    data.windows(signature.len()).position(|window| window == signature).unwrap()
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn test_nonempty_cd_comment() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::{CentralDirectoryReader, Entry};
    use crate::base::read::stream::ZipFileReader;
    use crate::tests::init_logger;

    init_logger();

    let data = include_bytes!("nonempty_cd_comment.zip").to_vec();

    let mut cursor = Cursor::new(data);

    let mut zip = ZipFileReader::new(&mut cursor);

    // Move forward through the ZIP's local file entries to reach the CD.
    // We do this instead of using the EOCDR locator to mimic a streaming read.
    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await.unwrap() {
        (.., zip) = entry.skip().await.unwrap();
        offset = zip.offset();
    }

    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset);

    let Entry::CentralDirectoryEntry(_) = cdr.next().await.unwrap() else {
        panic!("expected a central directory entry");
    };

    // Our position matches the end of the CD entry, including its
    // non-empty comment field.
    assert_eq!(cursor.position(), 0x2c + 52);
}

#[tokio::test]
async fn test_zip64_central_sentinel_requires_recognized_extra_field() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::CentralDirectoryReader;
    use crate::error::ZipError;
    use crate::spec::consts::CDH_SIGNATURE;

    let mut data = include_bytes!("../zip64/diff-002-sample.zip").to_vec();
    let signature_offset =
        data.windows(4).position(|bytes| bytes == CDH_SIGNATURE.to_le_bytes()).expect("central directory header");
    let filename_length =
        u16::from_le_bytes(data[signature_offset + 28..signature_offset + 30].try_into().unwrap()) as usize;
    let extra_field_offset = signature_offset + 46 + filename_length;
    assert_eq!(&data[extra_field_offset..extra_field_offset + 2], &1_u16.to_le_bytes());
    data[extra_field_offset..extra_field_offset + 2].copy_from_slice(&0xf00d_u16.to_le_bytes());

    let mut cursor = Cursor::new(&data[signature_offset + 4..]);
    let mut reader = CentralDirectoryReader::new(&mut cursor, signature_offset as u64);
    let err = match reader.next().await {
        Ok(_) => panic!("expected missing ZIP64 extended field"),
        Err(err) => err,
    };
    assert!(matches!(err, ZipError::Zip64ExtendedFieldIncomplete));
}

#[tokio::test]
async fn test_local_header_name_must_match_central_directory_name() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-004-sample.zip").to_vec();
    let reader = ZipFileReader::new(data).await.unwrap();

    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected local header name mismatch");
    };
    assert!(matches!(err, ZipError::LocalFileHeaderNameMismatch));
}

#[tokio::test]
async fn test_strong_encryption_entries_are_rejected() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-089-sample.zip").to_vec();

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected strong encryption to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("strong encryption")));
}

#[tokio::test]
async fn test_streamed_central_strong_encryption_entries_are_rejected() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::CentralDirectoryReader;
    use crate::error::ZipError;

    // `CentralDirectoryReader` starts immediately after the first central-directory signature.
    let mut record = [0; 42];
    record[4..6].copy_from_slice(&0x0040_u16.to_le_bytes());
    let mut reader = CentralDirectoryReader::new(Cursor::new(record), 0);

    let Err(err) = reader.next().await else {
        panic!("expected streamed strong encryption to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("strong encryption")));
}

fn strong_encryption_only_in_local_header() -> Vec<u8> {
    use crate::spec::consts::{CDH_SIGNATURE, LFH_SIGNATURE};

    let mut data = include_bytes!("diff-089-sample.zip").to_vec();
    let local_header = data.windows(4).position(|bytes| bytes == LFH_SIGNATURE.to_le_bytes()).unwrap();
    let central_header = data.windows(4).position(|bytes| bytes == CDH_SIGNATURE.to_le_bytes()).unwrap();

    // Set the strong-encryption flag in the local header, but clear it in the central-directory
    // record so that local-header parsing is solely responsible for rejecting the entry. Use
    // stored compression in both headers so this fixture works without compression features.
    data[local_header + 6..local_header + 8].copy_from_slice(&0x0040_u16.to_le_bytes());
    data[local_header + 8..local_header + 10].copy_from_slice(&0_u16.to_le_bytes());
    data[central_header + 8..central_header + 10].copy_from_slice(&0_u16.to_le_bytes());
    data[central_header + 10..central_header + 12].copy_from_slice(&0_u16.to_le_bytes());

    data
}

#[tokio::test]
async fn test_streamed_local_strong_encryption_entries_are_rejected() {
    use futures_lite::io::Cursor;

    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(Cursor::new(strong_encryption_only_in_local_header()));

    let Err(err) = reader.next_without_entry().await else {
        panic!("expected local strong encryption to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("strong encryption")));
}

#[tokio::test]
async fn test_seekable_local_strong_encryption_entries_are_rejected() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(strong_encryption_only_in_local_header()).await.unwrap();

    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected local strong encryption to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("strong encryption")));
}

#[tokio::test]
async fn test_archive_rejects_unsupported_central_directory_extract_versions() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let data = diff_092_data(20, UNSUPPORTED_EXTRACT_VERSION);

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected extract version to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("zip file version > 6.3")));
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn test_stream_rejects_unsupported_local_extract_versions() {
    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let data = diff_092_data(UNSUPPORTED_EXTRACT_VERSION, 20);

    let Err(err) = ZipFileReader::new(data.as_slice()).next_with_entry().await else {
        panic!("expected local extract version to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("zip file version > 6.3")));
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn test_stream_accepts_maximum_supported_local_extract_version() {
    use crate::base::read::stream::ZipFileReader;

    let data = diff_092_data(MAX_SUPPORTED_EXTRACT_VERSION, 20);

    assert!(ZipFileReader::new(data.as_slice()).next_with_entry().await.unwrap().is_some());
}

#[tokio::test]
async fn test_incremental_central_directory_reader_rejects_unsupported_extract_versions() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::CentralDirectoryReader;
    use crate::error::ZipError;

    let data = diff_092_data(20, UNSUPPORTED_EXTRACT_VERSION);
    let offset = central_directory_offset(&data);
    let mut cursor = Cursor::new(&data[offset + 4..]);
    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset as u64);

    let Err(err) = cdr.next().await else {
        panic!("expected extract version to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("zip file version > 6.3")));
}

#[tokio::test]
async fn test_incremental_central_directory_reader_accepts_maximum_supported_extract_version() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::{CentralDirectoryReader, Entry};

    let data = diff_092_data(20, MAX_SUPPORTED_EXTRACT_VERSION);
    let offset = central_directory_offset(&data);
    let mut cursor = Cursor::new(&data[offset + 4..]);
    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset as u64);

    assert!(matches!(cdr.next().await.unwrap(), Entry::CentralDirectoryEntry(_)));
}

/// Verifies that a streamed read rejects a physical central-directory entry when the ordinary
/// EOCD record declares a central-directory byte span one byte shorter than the actual entry.
#[tokio::test]
async fn test_streamed_central_directory_size_must_match_end_record() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::{CentralDirectoryReader, Entry};
    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-094-sample.zip").to_vec();
    let mut cursor = Cursor::new(data);
    let mut zip = ZipFileReader::new(&mut cursor);

    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await.unwrap() {
        (.., zip) = entry.skip().await.unwrap();
        offset = zip.offset();
    }

    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset);
    assert!(matches!(cdr.next().await.unwrap(), Entry::CentralDirectoryEntry(_)));

    let Err(err) = cdr.next().await else {
        panic!("expected central-directory size mismatch");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectorySize { .. }));
}

/// Verifies that a streamed read rejects a physical central-directory entry when the ordinary
/// EOCD record declares a zero-byte central directory.
#[tokio::test]
async fn test_streamed_zero_central_directory_size_must_match_end_record() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::{CentralDirectoryReader, Entry};
    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("zero-central-directory-size.zip").to_vec();

    let mut cursor = Cursor::new(data);
    let mut zip = ZipFileReader::new(&mut cursor);

    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await.unwrap() {
        (.., zip) = entry.skip().await.unwrap();
        offset = zip.offset();
    }

    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset);
    assert!(matches!(cdr.next().await.unwrap(), Entry::CentralDirectoryEntry(_)));

    let Err(err) = cdr.next().await else {
        panic!("expected central-directory size mismatch");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectorySize { expected: 0, .. }));
}

#[tokio::test]
async fn test_streamed_zip64_central_directory_size_must_match_end_record() {
    use futures_lite::io::Cursor;

    use crate::base::read::cd::{CentralDirectoryReader, Entry};
    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;
    use crate::spec::consts::ZIP64_EOCDR_SIGNATURE;

    let mut data = include_bytes!("../zip64/zip64.zip").to_vec();
    let zip64_eocdr_offset = data.windows(4).position(|bytes| bytes == ZIP64_EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    data[zip64_eocdr_offset + 40..zip64_eocdr_offset + 48].copy_from_slice(&0_u64.to_le_bytes());

    let mut cursor = Cursor::new(data);
    let mut zip = ZipFileReader::new(&mut cursor);

    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await.unwrap() {
        (.., zip) = entry.skip().await.unwrap();
        offset = zip.offset();
    }

    let mut cdr = CentralDirectoryReader::new(&mut cursor, offset);
    assert!(matches!(cdr.next().await.unwrap(), Entry::CentralDirectoryEntry(_)));

    let Err(err) = cdr.next().await else {
        panic!("expected ZIP64 central-directory size mismatch");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectorySize { expected: 0, .. }));
}
