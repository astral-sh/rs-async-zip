// Copyright (c) 2025 Astral
// MIT License (https://github.com/astral-sh/rs-async-zip/blob/main/LICENSE)

use crate::spec::version::MAX_SUPPORTED_EXTRACT_VERSION;

const UNSUPPORTED_EXTRACT_VERSION: u16 = MAX_SUPPORTED_EXTRACT_VERSION + 1;

// Keep fixture names in assertion failures without duplicating their paths.
macro_rules! fixture {
    ($name:literal) => {
        ($name, include_bytes!(concat!("fixtures/", $name)).as_slice())
    };
}

/// Consume both streaming phases, including the terminal end record.
pub(super) async fn read_streamed_archive<R: futures_lite::io::AsyncBufRead + Unpin>(
    mut reader: R,
) -> crate::error::Result<()> {
    use crate::base::read::{cd::CentralDirectoryReader, cd::Entry, stream::ZipFileReader};

    let mut zip = ZipFileReader::new(&mut reader);
    let mut offset = 0;
    while let Some(entry) = zip.next_with_entry().await? {
        (.., zip) = entry.skip().await?;
        offset = zip.offset();
    }
    let mut directory = CentralDirectoryReader::new(&mut reader, offset);
    loop {
        if let Entry::EndOfCentralDirectoryRecord { .. } = directory.next().await? {
            return Ok(());
        }
    }
}

async fn archive_results(data: &[u8]) -> [crate::error::Result<()>; 3] {
    use crate::base::read::{mem, seek};
    use futures_lite::io::{BufReader, Cursor};

    // Exercise short reads in both I/O paths; the memory reader covers ordinary reads.
    let source = || BufReader::new(super::ShortReader::new(Cursor::new(data), 3));
    [
        read_streamed_archive(source()).await,
        seek::ZipFileReader::new(source()).await.map(|_| ()),
        mem::ZipFileReader::new(data.to_vec()).await.map(|_| ()),
    ]
}

fn assert_trailing_contents(results: impl IntoIterator<Item = crate::error::Result<()>>) {
    for result in results {
        assert!(matches!(result, Err(crate::error::ZipError::TrailingContents)), "{result:?}");
    }
}

#[tokio::test]
async fn malo_iffy_suffix_not_comment() {
    assert_trailing_contents(archive_results(include_bytes!("../malo/iffy/suffix_not_comment.zip")).await);

    #[cfg(feature = "tokio-fs")]
    assert_trailing_contents([crate::tokio::read::fs::ZipFileReader::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/read/malo/iffy/suffix_not_comment.zip"
    ))
    .await
    .map(|_| ())]);
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn malo_reject_dupe_eocd() {
    use crate::error::ZipError;

    let [streaming, seeking, memory] = archive_results(include_bytes!("../malo/reject/dupe_eocd.zip")).await;
    assert_trailing_contents([streaming]);
    // Seeking already rejects this fixture: its declared CD span includes the first EOCD.
    for result in [seeking, memory] {
        assert!(matches!(result, Err(ZipError::InvalidCentralDirectorySize { expected: 73, actual: 51 })));
    }
}

#[tokio::test]
async fn malo_accept_store() {
    let data = include_bytes!("../malo/accept/store.zip");
    for result in archive_results(data).await.into_iter().chain(extraction_results(data).await) {
        result.unwrap();
    }
}

#[tokio::test]
async fn malo_accept_comment() {
    let data = include_bytes!("../malo/accept/comment.zip");
    for result in archive_results(data).await.into_iter().chain(extraction_results(data).await) {
        result.unwrap();
    }
}

#[tokio::test]
async fn malo_iffy_8bitcomment() {
    use crate::error::ZipError;

    let [streaming, seeking, memory] = archive_results(include_bytes!("../malo/iffy/8bitcomment.zip")).await;
    assert!(matches!(streaming, Err(ZipError::ZipInZip)), "{streaming:?}");
    // Seeking already rejects the end record found inside this fixture's binary comment.
    for result in [seeking, memory] {
        assert!(matches!(result, Err(ZipError::FeatureNotSupported("Spanned/split files"))));
    }
}

#[tokio::test]
async fn malo_malicious_zipinzip() {
    use crate::error::ZipError;

    let [streaming, seeking, memory] = archive_results(include_bytes!("../malo/malicious/zipinzip.zip")).await;
    assert!(matches!(streaming, Err(ZipError::ZipInZip)), "{streaming:?}");
    // This unchanged fixture already fails seeking's directory binding check. The boundary tests
    // also cover a generated archive with self-consistent offsets inside the containing source.
    for result in [seeking, memory] {
        assert!(matches!(result, Err(ZipError::InvalidCentralDirectoryBinding { directory_end: 87, end_record: 196 })));
    }
}

#[tokio::test]
async fn entry_comments_are_validated() {
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder, ZipString};

    // Malo covers archive comments; use the writer to exercise entry comments and both encodings
    // of an Info-ZIP Unicode comment. These bytes remain valid in entry payloads.
    for (comment, rejected) in [
        (ZipString::new_with_alternative("\0\t\n".into(), vec![0xff]), false),
        ("\x01\x08".into(), true),
        (ZipString::new_with_alternative("safe".into(), vec![0x01]), true),
        (ZipString::new_with_alternative("\x08".into(), b"safe".to_vec()), true),
    ] {
        let mut writer = ZipFileWriter::new(Vec::new());
        writer
            .write_entry_whole(ZipEntryBuilder::new("entry".into(), Compression::Stored).comment(comment), b"\x01\x08")
            .await
            .unwrap();
        let data = writer.close().await.unwrap();
        let results = archive_results(&data).await;
        if rejected {
            assert!(results.iter().all(|result| matches!(result, Err(ZipError::ZipInZip))), "{results:?}");
        } else {
            for result in results {
                result.unwrap();
            }
        }
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn malo_accept_deflate() {
    for data in [
        include_bytes!("../malo/accept/deflate.zip").as_slice(),
        include_bytes!("../malo/accept/normal_deflate_zip64_extra.zip").as_slice(),
    ] {
        for result in archive_results(data).await.into_iter().chain(extraction_results(data).await) {
            result.unwrap();
        }
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn malo_accept_zip64_eocd() {
    let data = include_bytes!("../malo/accept/zip64_eocd.zip");
    for result in archive_results(data).await.into_iter().chain(extraction_results(data).await) {
        result.unwrap();
    }
}

#[tokio::test]
async fn trailing_nul_padding_limit_is_enforced() {
    for ((name, data), accepted) in [
        (fixture!("stored.zip"), true),
        (fixture!("stored-padding-4096.zip"), true),
        (fixture!("stored-padding-4097.zip"), false),
        #[cfg(feature = "deflate")]
        (fixture!("zip64.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("zip64-padding-4096.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("zip64-padding-4097.zip"), false),
    ] {
        for result in archive_results(data).await {
            assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
        }
    }
}

#[tokio::test]
async fn trailing_nul_validation_has_bounded_io() {
    use crate::base::read::io::validate_trailing_contents;
    use crate::error::ZipError;
    use futures_lite::io::{repeat, AsyncReadExt};

    // Short reads must accumulate toward the cap without reaching the artificial EOF.
    let mut reader = super::ShortReader::new(repeat(0).take(8192), 3).with_eof_error(std::io::ErrorKind::Other);
    let result = validate_trailing_contents(&mut reader).await;
    assert!(matches!(result, Err(ZipError::TrailingContents)), "{result:?}");
    assert_eq!(reader.bytes_read, 4097);
}

#[tokio::test]
async fn nonzero_suffix_after_nul_padding_is_rejected() {
    for (_, data) in [
        fixture!("stored-nonzero-suffix.zip"),
        #[cfg(feature = "deflate")]
        fixture!("zip64-nonzero-suffix.zip"),
    ] {
        assert_trailing_contents(archive_results(data).await);
    }
}

#[tokio::test]
async fn suffix_read_errors_are_preserved() {
    use crate::base::read::seek::ZipFileReader;
    use crate::error::ZipError;
    use futures_lite::io::{BufReader, Cursor};
    use std::io::ErrorKind;

    let data = include_bytes!("../malo/accept/store.zip");
    let source = || {
        BufReader::new(super::ShortReader::new(Cursor::new(&data[..]), 3).with_eof_error(ErrorKind::ConnectionReset))
    };
    let streaming = read_streamed_archive(source()).await;
    let seeking = ZipFileReader::new(source()).await.map(|_| ());
    for result in [streaming, seeking] {
        let ZipError::UpstreamReadError(error) = result.unwrap_err() else {
            panic!("expected the suffix I/O error");
        };
        assert_eq!(error.kind(), ErrorKind::ConnectionReset);
    }
}

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

#[cfg(feature = "tokio-fs")]
struct FixtureFile(std::path::PathBuf);

#[cfg(feature = "tokio-fs")]
impl FixtureFile {
    fn new(data: &[u8]) -> Self {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("async-zip-boundaries-{}.zip", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        let fixture = Self(path);
        file.write_all(data).unwrap();
        fixture
    }
}

#[cfg(feature = "tokio-fs")]
impl Drop for FixtureFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn extraction_results(data: &[u8]) -> Vec<crate::error::Result<()>> {
    use crate::base::read::{mem, seek};
    use futures_lite::io::{copy, sink, AsyncBufRead, AsyncSeek, BufReader, Cursor};

    async fn extract_seek<R: AsyncBufRead + AsyncSeek + Unpin>(
        source: R,
        with_entry: bool,
    ) -> crate::error::Result<()> {
        let mut reader = seek::ZipFileReader::new(source).await?;
        for index in 0..reader.file().entries().len() {
            if with_entry {
                copy(&mut reader.reader_with_entry(index).await?, &mut sink()).await?;
            } else {
                copy(&mut reader.reader_without_entry(index).await?, &mut sink()).await?;
            }
        }
        Ok(())
    }

    // Complete ordinary entry reads, including directory entries. No separate validation call.
    let mut results = vec![
        async {
            let file = seek::ZipFileReader::new(Cursor::new(data)).await?.file().clone();
            for index in 0..file.entries().len() {
                let reader = seek::ZipFileReader::from_raw_parts(Cursor::new(data), file.clone());
                copy(&mut reader.into_entry(index).await?, &mut sink()).await?;
            }
            Ok(())
        }
        .await,
    ];
    for with_entry in [false, true] {
        results.push(extract_seek(Cursor::new(data), with_entry).await);
        let short = BufReader::with_capacity(3, super::ShortReader::new(Cursor::new(data), 3).with_pending());
        results.push(extract_seek(short, with_entry).await);
        results.push(
            async {
                let reader = mem::ZipFileReader::new(data.to_vec()).await?;
                for index in 0..reader.file().entries().len() {
                    if with_entry {
                        copy(&mut reader.reader_with_entry(index).await?, &mut sink()).await?;
                    } else {
                        copy(&mut reader.reader_without_entry(index).await?, &mut sink()).await?;
                    }
                }
                Ok(())
            }
            .await,
        );
        #[cfg(feature = "tokio-fs")]
        {
            let fixture = FixtureFile::new(data);
            results.push(
                async {
                    let reader = crate::tokio::read::fs::ZipFileReader::new(&fixture.0).await?;
                    for index in 0..reader.file().entries().len() {
                        if with_entry {
                            copy(&mut reader.reader_with_entry(index).await?, &mut sink()).await?;
                        } else {
                            copy(&mut reader.reader_without_entry(index).await?, &mut sink()).await?;
                        }
                    }
                    Ok(())
                }
                .await,
            );
        }
    }
    results
}

#[tokio::test]
async fn entry_validation_allows_nested_zip_payloads() {
    use crate::base::read::seek::ZipFileReader;
    use futures_lite::io::{BufReader, Cursor};

    let data = include_bytes!("fixtures/nested-payload.zip");
    let source = BufReader::new(super::ShortReader::new(Cursor::new(data), usize::MAX));
    let mut reader = ZipFileReader::new(source).await.unwrap();
    reader.inner_mut().get_mut().bytes_read = 0;
    let mut actual = Vec::new();
    reader.reader_with_entry(0).await.unwrap().read_to_end_checked(&mut actual).await.unwrap();
    let nested = include_bytes!("fixtures/stored.zip");
    assert_eq!(actual.len(), 2 * 1024 * 1024);
    assert_eq!(&actual[..nested.len()], nested);
    assert!(actual[nested.len()..].iter().all(|&byte| byte == 0));
    let source = reader.into_inner().into_inner();
    assert!(source.bytes_read <= actual.len() + 8_192, "entry validation reread its payload");
}

#[tokio::test]
async fn zip_in_comment_is_rejected_by_default() {
    let results = extraction_results(include_bytes!("fixtures/zip-in-comment.zip")).await;
    assert!(results.iter().all(Result::is_err), "{results:?}");
}

#[tokio::test]
async fn concatenated_archives_are_rejected_by_default() {
    for (name, data) in
        [fixture!("concatenated.zip"), fixture!("concatenated-large.zip"), fixture!("concatenated-from-zero.zip")]
    {
        let results = extraction_results(data).await;
        assert!(results.iter().all(Result::is_err), "{name}: {results:?}");
    }
}

#[tokio::test]
async fn embedded_empty_archive_is_rejected_by_default() {
    let results = extraction_results(include_bytes!("fixtures/empty-in-comment.zip")).await;
    assert!(results.iter().all(Result::is_err), "{results:?}");
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn malo_iffy_prefix() {
    assert!(archive_results(include_bytes!("../malo/iffy/prefix.zip")).await.iter().all(Result::is_err));
}

#[tokio::test]
async fn prefixed_archive_is_rejected_by_default() {
    let results = extraction_results(include_bytes!("fixtures/prefix.zip")).await;
    assert!(results.iter().all(Result::is_err), "{results:?}");
}

#[tokio::test]
async fn entry_validation_rejects_a_gap_before_the_directory() {
    let results = extraction_results(include_bytes!("fixtures/gap-before-directory.zip")).await;
    assert!(results.iter().all(Result::is_err), "{results:?}");
}

#[tokio::test]
async fn entry_validation_rejects_local_size_overflow_and_overlap() {
    for (name, data) in [fixture!("local-size-overflow.zip"), fixture!("local-size-overlap.zip")] {
        let results = extraction_results(data).await;
        assert!(results.iter().all(Result::is_err), "{name}: {results:?}");
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn malo_descriptor_flags_must_match() {
    // These original Malo files disagree on the local and central descriptor flags.
    for data in [
        include_bytes!("../malo/accept/data_descriptor.zip").as_slice(),
        include_bytes!("../malo/accept/data_descriptor_zip64.zip").as_slice(),
    ] {
        for result in extraction_results(data).await {
            assert!(matches!(result, Err(crate::error::ZipError::LocalFileHeaderDataDescriptorMismatch)));
        }
    }
}

#[tokio::test]
async fn entry_validation_accepts_only_descriptor_sized_gaps() {
    for ((name, data), accepted) in [
        (fixture!("descriptor-stored-signed.zip"), true),
        (fixture!("descriptor-stored-unsigned.zip"), true),
        (fixture!("descriptor-stored-missing.zip"), false),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-signed.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-unsigned.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-missing.zip"), false),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-zip64-signed.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-zip64-unsigned.zip"), true),
        #[cfg(feature = "deflate")]
        (fixture!("descriptor-deflate-zip64-missing.zip"), false),
    ] {
        for result in extraction_results(data).await {
            assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
        }
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn entry_validation_requires_an_unambiguous_descriptor_length() {
    for (name, data) in [fixture!("descriptor-index-missing.zip"), fixture!("descriptor-index-conflict.zip")] {
        let results = extraction_results(data).await;
        assert!(results.iter().all(Result::is_err), "{name}: {results:?}");
    }
}

#[tokio::test]
async fn entry_validation_rejects_corrupt_descriptor_fields() {
    for (name, data) in [
        fixture!("descriptor-stored-signed-bad-signature.zip"),
        fixture!("descriptor-stored-signed-bad-crc.zip"),
        fixture!("descriptor-stored-signed-bad-compressed-size.zip"),
        fixture!("descriptor-stored-signed-bad-uncompressed-size.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-signed-bad-signature.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-signed-bad-crc.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-signed-bad-compressed-size.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-signed-bad-uncompressed-size.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-zip64-signed-bad-signature.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-zip64-signed-bad-crc.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-zip64-signed-bad-compressed-size.zip"),
        #[cfg(feature = "deflate")]
        fixture!("descriptor-deflate-zip64-signed-bad-uncompressed-size.zip"),
    ] {
        let results = extraction_results(data).await;
        assert!(results.iter().all(Result::is_err), "{name}: {results:?}");
    }
}

#[tokio::test]
async fn entry_validation_checks_stored_data_descriptors() {
    use crate::base::read::seek::ZipFileReader;
    use crate::spec::consts::DATA_DESCRIPTOR_SIGNATURE;
    use futures_lite::io::{AsyncReadExt, BufReader, Cursor};

    // A CRC can equal the optional signature. The record length must disambiguate it.
    let signature_crc_payload = b"\xac\x0a\x7a\xd5";
    assert_eq!(crc32fast::hash(signature_crc_payload), DATA_DESCRIPTOR_SIGNATURE);
    for ((name, data), (_, corrupt), payload) in [
        (
            fixture!("descriptor-stored-signed.zip"),
            fixture!("descriptor-stored-signed-bad-uncompressed-size.zip"),
            b"hello".as_slice(),
        ),
        (
            fixture!("descriptor-stored-unsigned.zip"),
            fixture!("descriptor-stored-unsigned-bad-uncompressed-size.zip"),
            b"hello",
        ),
        (
            fixture!("descriptor-stored-empty-signed.zip"),
            fixture!("descriptor-stored-empty-signed-bad-uncompressed-size.zip"),
            b"",
        ),
        (
            fixture!("descriptor-stored-empty-unsigned.zip"),
            fixture!("descriptor-stored-empty-unsigned-bad-uncompressed-size.zip"),
            b"",
        ),
        (
            fixture!("descriptor-stored-signature-crc-signed.zip"),
            fixture!("descriptor-stored-signature-crc-signed-bad-uncompressed-size.zip"),
            signature_crc_payload,
        ),
        (
            fixture!("descriptor-stored-signature-crc-unsigned.zip"),
            fixture!("descriptor-stored-signature-crc-unsigned-bad-uncompressed-size.zip"),
            signature_crc_payload,
        ),
    ] {
        for result in extraction_results(data).await {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
        let directory = central_directory_offset(data);
        let mut reader = ZipFileReader::new(Cursor::new(data)).await.unwrap();
        let file = reader.file().clone();
        let mut entry = reader.reader_with_entry(0).await.unwrap();
        let mut actual = Vec::new();
        entry.read_to_end_checked(&mut actual).await.unwrap();
        assert_eq!(actual, payload);
        assert_eq!(entry.bytes_read(), payload.len() as u64);
        assert_eq!(entry.read(&mut [0; 1]).await.unwrap(), 0);
        drop(entry);
        assert_eq!(reader.into_inner().position(), directory as u64);

        // A source that fails while finishing the descriptor must not report entry EOF.
        for error in [std::io::ErrorKind::UnexpectedEof, std::io::ErrorKind::BrokenPipe] {
            let source = super::ShortReader::new(Cursor::new(&data[..directory - 1]), 1).with_eof_error(error);
            let mut reader = ZipFileReader::from_raw_parts(BufReader::with_capacity(1, source), file.clone());
            let mut entry = reader.reader_without_entry(0).await.unwrap();
            assert_eq!(entry.read_to_end(&mut Vec::new()).await.unwrap_err().kind(), error);
        }

        // Partial reads need not validate an unread descriptor, but an ordinary read to EOF
        // must do so. Empty reads must not trigger completion or consume descriptor bytes.
        let results = extraction_results(corrupt).await;
        assert!(results.iter().all(Result::is_err), "{name}: {results:?}");
        let source = BufReader::with_capacity(1, super::ShortReader::new(Cursor::new(corrupt), 1).with_pending());
        let mut reader = ZipFileReader::new(source).await.unwrap();
        let mut entry = reader.reader_without_entry(0).await.unwrap();
        assert_eq!(entry.read(&mut []).await.unwrap(), 0);
        entry.read_exact(&mut vec![0; payload.len()]).await.unwrap();
        assert_eq!(entry.read(&mut []).await.unwrap(), 0);
        assert!(entry.read(&mut [0; 1]).await.is_err(), "corrupt descriptor accepted at EOF");
        assert!(entry.read(&mut [0; 1]).await.is_err(), "descriptor error was lost on a repeated read");
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn entry_completion_rejects_unconsumed_compressed_bytes() {
    // Inflate stops at the end of its stream. A declared compressed size that also covers
    // junk must not make that junk count as validated entry data.
    let results = extraction_results(include_bytes!("fixtures/deflate-with-junk.zip")).await;
    assert!(results.iter().all(Result::is_err), "{results:?}");
}

#[tokio::test]
async fn entry_completion_rejects_truncated_stored_payload() {
    use crate::base::read::seek::ZipFileReader;
    use futures_lite::io::{AsyncReadExt, Cursor};

    let data = include_bytes!("fixtures/stored.zip");
    let file = ZipFileReader::new(Cursor::new(data)).await.unwrap().file().clone();
    // Simulate a source ending before the data promised by its previously parsed directory.
    let mut reader = ZipFileReader::from_raw_parts(Cursor::new(include_bytes!("fixtures/stored-truncated.zip")), file);
    let mut entry = reader.reader_without_entry(0).await.unwrap();
    assert_eq!(entry.read(&mut []).await.unwrap(), 0);
    let error = entry.read_to_end(&mut Vec::new()).await.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(entry.read(&mut [0; 1]).await.is_err());
}

#[tokio::test]
async fn entry_validation_accepts_subdir_in_either_directory_order() {
    for (name, data) in [
        ("Malo subdir", include_bytes!("../malo/accept/subdir.zip").as_slice()),
        fixture!("subdir.zip"),
        fixture!("subdir-reordered.zip"),
    ] {
        for result in extraction_results(data).await {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }
}

#[tokio::test]
async fn entry_validation_accepts_empty_archives() {
    assert_trailing_contents(extraction_results(include_bytes!("fixtures/empty-with-suffix.zip")).await);
    for (name, data) in [fixture!("empty.zip"), fixture!("empty-zip64.zip")] {
        for result in extraction_results(data).await {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }
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
async fn test_archive_accepts_nonzero_reserved_extract_version_bytes() {
    use crate::base::read::mem::ZipFileReader;

    let version = 3 << 8 | 20;
    let data = diff_092_data(version, version);

    let reader = ZipFileReader::new(data).await.unwrap();
    reader.reader_without_entry(0).await.unwrap();
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
    use crate::spec::consts::{EOCDR_SIGNATURE, ZIP64_EOCDR_SIGNATURE};

    let mut data = include_bytes!("../zip64/zip64.zip").to_vec();
    let zip64_eocdr_offset = data.windows(4).position(|bytes| bytes == ZIP64_EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    data[zip64_eocdr_offset + 24..zip64_eocdr_offset + 40].fill(0);
    data[zip64_eocdr_offset + 40..zip64_eocdr_offset + 48].copy_from_slice(&0_u64.to_le_bytes());
    let eocdr_offset = data.windows(4).position(|bytes| bytes == EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    data[eocdr_offset + 8..eocdr_offset + 12].fill(0xff);
    data[eocdr_offset + 12..eocdr_offset + 16].copy_from_slice(&u32::MAX.to_le_bytes());

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

#[tokio::test]
async fn test_nul_filenames_are_rejected() {
    use futures_lite::io::Cursor;

    use crate::base::read::mem;
    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-096-sample.zip").to_vec();

    let Err(err) = mem::ZipFileReader::new(data.clone()).await else {
        panic!("expected an embedded NUL filename to be rejected");
    };
    let ZipError::FileNameContainsNul { filename } = err else {
        panic!("expected an embedded NUL filename error");
    };
    assert!(filename.contains(&0));

    let mut zip = ZipFileReader::new(Cursor::new(data));
    loop {
        match zip.next_with_entry().await {
            Err(err) => {
                let ZipError::FileNameContainsNul { filename } = err else {
                    panic!("expected an embedded NUL filename error");
                };
                assert!(filename.contains(&0));
                break;
            }
            Ok(Some(entry)) => {
                (.., zip) = entry.skip().await.unwrap();
            }
            Ok(None) => panic!("expected an embedded NUL filename to be rejected while streaming"),
        }
    }
}

fn empty_stored_zip(
    local_flags: u16,
    local_compressed_size: u32,
    local_uncompressed_size: u32,
    local_extra: &[u8],
    central_flags: u16,
    central_compressed_size: u32,
    central_uncompressed_size: u32,
) -> Vec<u8> {
    let mut zip = Vec::new();

    // Local file header for an empty stored entry named "a".
    zip.extend_from_slice(b"PK\x03\x04");
    zip.extend_from_slice(&20_u16.to_le_bytes());
    zip.extend_from_slice(&local_flags.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u32.to_le_bytes());
    zip.extend_from_slice(&local_compressed_size.to_le_bytes());
    zip.extend_from_slice(&local_uncompressed_size.to_le_bytes());
    zip.extend_from_slice(&1_u16.to_le_bytes());
    zip.extend_from_slice(&(local_extra.len() as u16).to_le_bytes());
    zip.push(b'a');
    zip.extend_from_slice(local_extra);

    let central_directory_offset = zip.len() as u32;

    zip.extend_from_slice(b"PK\x01\x02");
    zip.extend_from_slice(&20_u16.to_le_bytes());
    zip.extend_from_slice(&20_u16.to_le_bytes());
    zip.extend_from_slice(&central_flags.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u32.to_le_bytes());
    zip.extend_from_slice(&central_compressed_size.to_le_bytes());
    zip.extend_from_slice(&central_uncompressed_size.to_le_bytes());
    zip.extend_from_slice(&1_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u32.to_le_bytes());
    zip.extend_from_slice(&0_u32.to_le_bytes());
    zip.push(b'a');

    let central_directory_size = zip.len() as u32 - central_directory_offset;

    zip.extend_from_slice(b"PK\x05\x06");
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());
    zip.extend_from_slice(&1_u16.to_le_bytes());
    zip.extend_from_slice(&1_u16.to_le_bytes());
    zip.extend_from_slice(&central_directory_size.to_le_bytes());
    zip.extend_from_slice(&central_directory_offset.to_le_bytes());
    zip.extend_from_slice(&0_u16.to_le_bytes());

    zip
}

fn zip64_sizes_extra_field(compressed_size: u64, uncompressed_size: u64) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend_from_slice(&1_u16.to_le_bytes());
    extra.extend_from_slice(&16_u16.to_le_bytes());
    extra.extend_from_slice(&uncompressed_size.to_le_bytes());
    extra.extend_from_slice(&compressed_size.to_le_bytes());
    extra
}

#[tokio::test]
async fn test_local_header_sizes_must_match_central_directory() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(empty_stored_zip(0, 1, 1, &[], 0, 0, 0)).await.unwrap();
    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected local header sizes to be rejected");
    };

    assert!(matches!(err, ZipError::LocalFileHeaderSizeMismatch));
}

#[tokio::test]
async fn test_local_header_descriptor_flag_must_match_central_directory() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(empty_stored_zip(1 << 3, 0, 0, &[], 0, 0, 0)).await.unwrap();
    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected local header descriptor flag to be rejected");
    };

    assert!(matches!(err, ZipError::LocalFileHeaderDataDescriptorMismatch));
}

#[tokio::test]
async fn test_each_concrete_local_size_must_match_central_directory() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(empty_stored_zip(0, u32::MAX, 1, &[], 0, 0, 0)).await.unwrap();
    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected the concrete local uncompressed size to be rejected");
    };

    assert!(matches!(err, ZipError::LocalFileHeaderSizeMismatch));
}

#[tokio::test]
async fn test_local_zip64_sizes_must_match_central_directory() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let local_extra = zip64_sizes_extra_field(1, 0);
    let reader = ZipFileReader::new(empty_stored_zip(0, u32::MAX, u32::MAX, &local_extra, 0, 0, 0)).await.unwrap();
    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected local ZIP64 sizes to be rejected");
    };

    assert!(matches!(err, ZipError::LocalFileHeaderSizeMismatch));
}

#[tokio::test]
async fn test_local_zip64_sizes_must_have_overrides() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let reader = ZipFileReader::new(empty_stored_zip(0, u32::MAX, u32::MAX, &[], 0, 0, 0)).await.unwrap();
    let Err(err) = reader.reader_without_entry(0).await else {
        panic!("expected absent local ZIP64 sizes to be rejected");
    };

    assert!(matches!(err, ZipError::LocalFileHeaderSizeMismatch));
}

#[tokio::test]
async fn test_each_local_zip64_size_must_have_an_override() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    for (compressed_size, uncompressed_size) in [(u32::MAX, 0), (0, u32::MAX)] {
        let reader =
            ZipFileReader::new(empty_stored_zip(0, compressed_size, uncompressed_size, &[], 0, 0, 0)).await.unwrap();
        let Err(err) = reader.reader_without_entry(0).await else {
            panic!("expected absent local ZIP64 size to be rejected");
        };

        assert!(matches!(err, ZipError::LocalFileHeaderSizeMismatch));
    }
}

#[tokio::test]
async fn test_central_directory_encryption_is_rejected() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-085-sample.zip").to_vec();

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected central-directory encryption to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("encryption")));
}

#[tokio::test]
async fn test_compressed_patched_entries_are_rejected() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let data = include_bytes!("diff-088-sample.zip").to_vec();

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected compressed patched data to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("compressed patched data")));
}

#[tokio::test]
async fn test_stream_with_entry_rejects_compressed_patched_local_headers() {
    use futures_lite::io::Cursor;

    use crate::base::read::stream::ZipFileReader;
    use crate::error::ZipError;

    let mut data = include_bytes!("diff-088-sample.zip").to_vec();
    data[6] |= 0x20;
    let zip = ZipFileReader::new(Cursor::new(data));

    let Err(err) = zip.next_with_entry().await else {
        panic!("expected compressed patched data in the local header to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("compressed patched data")));
}

#[tokio::test]
async fn test_seekable_reader_rejects_compressed_patched_local_headers() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let mut data = include_bytes!("diff-088-sample.zip").to_vec();
    let central_directory_offset =
        data.windows(4).position(|window| window == [0x50, 0x4b, 0x01, 0x02]).expect("central directory record");

    data[6] |= 0x20;
    data[central_directory_offset + 8] &= !0x20;
    data[central_directory_offset + 10..central_directory_offset + 12].copy_from_slice(&0u16.to_le_bytes());

    let zip = ZipFileReader::new(data).await.expect("central directory should be valid");
    let Err(err) = zip.reader_without_entry(0).await else {
        panic!("expected compressed patched data in the local header to be rejected");
    };
    assert!(matches!(err, ZipError::FeatureNotSupported("compressed patched data")));
}

#[tokio::test]
async fn test_entry_body_must_not_overlap_later_local_header() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("a".into(), Compression::Stored), b"").await.unwrap();
    writer.write_entry_whole(ZipEntryBuilder::new("b".into(), Compression::Stored), b"").await.unwrap();
    writer.close().await.unwrap();

    let central_directory =
        data.windows(4).position(|window| window == b"PK\x01\x02").expect("expected central directory");
    data[central_directory + 20..central_directory + 24].copy_from_slice(&1_u32.to_le_bytes());

    let zip = ZipFileReader::new(data).await.expect("central directory should be valid");
    let Err(err) = zip.reader_without_entry(0).await else {
        panic!("expected overlapping entry range");
    };
    assert!(matches!(err, ZipError::EntryDataRangeOverlap { .. }));
}

#[tokio::test]
async fn test_directory_size_must_cover_claimed_entry_count() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder};

    let mut writer = ZipFileWriter::new(Vec::new());
    writer.write_entry_whole(ZipEntryBuilder::new("alpha.txt".into(), Compression::Stored), b"alpha\n").await.unwrap();
    writer.write_entry_whole(ZipEntryBuilder::new("beta.txt".into(), Compression::Stored), b"beta\n").await.unwrap();
    let mut data = writer.close().await.unwrap();

    // Clear the EOCD central-directory size while leaving the two entry counts intact.
    let eocd_offset = data.len() - 22;
    data[eocd_offset + 12..eocd_offset + 16].fill(0);

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected invalid central directory entry count");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectoryEntryCount { entries: 2 }));
}

#[tokio::test]
async fn test_directory_size_must_cover_variable_length_entries() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder};

    let mut writer = ZipFileWriter::new(Vec::new());
    writer
        .write_entry_whole(ZipEntryBuilder::new("long-alpha.txt".into(), Compression::Stored), b"alpha\n")
        .await
        .unwrap();
    writer
        .write_entry_whole(ZipEntryBuilder::new("long-beta.txt".into(), Compression::Stored), b"beta\n")
        .await
        .unwrap();
    let mut data = writer.close().await.unwrap();

    // This size covers two fixed CD headers, but not their filename fields.
    let eocd_offset = data.len() - 22;
    data[eocd_offset + 12..eocd_offset + 16].copy_from_slice(&92_u32.to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected invalid central directory entry count");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectoryEntryCount { entries: 2 }));
}

#[tokio::test]
async fn test_local_extra_field_must_not_overlap_later_local_header() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("a".into(), Compression::Stored), b"").await.unwrap();
    writer.write_entry_whole(ZipEntryBuilder::new("b".into(), Compression::Stored), b"").await.unwrap();
    writer.close().await.unwrap();

    // Consume the following local file header signature as a local-only extra field.
    data[28..30].copy_from_slice(&4_u16.to_le_bytes());

    let zip = ZipFileReader::new(data).await.expect("central directory should be valid");
    let Err(err) = zip.reader_without_entry(0).await else {
        panic!("expected overlapping local header range");
    };
    assert!(matches!(err, ZipError::EntryDataRangeOverlap { .. }));
}

#[tokio::test]
async fn test_many_entry_ranges_validate() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    for index in 0..1_024 {
        // Cover both empty entries and payloads that fit in any nonempty read-ahead buffer.
        let payload: &[u8] = if index % 2 == 0 { b"" } else { b"x" };
        writer
            .write_entry_whole(ZipEntryBuilder::new(format!("{index}").into(), Compression::Stored), payload)
            .await
            .unwrap();
    }
    writer.close().await.unwrap();

    let source =
        futures_lite::io::BufReader::new(super::ShortReader::new(futures_lite::io::Cursor::new(&data[..]), usize::MAX));
    let mut seeking = crate::base::read::seek::ZipFileReader::new(source).await.unwrap();
    let source = seeking.inner_mut().get_mut();
    assert!(source.seeks < 32, "construction performed {} seeks for 1,024 entries", source.seeks);
    source.seeks = 0;
    for index in 0..1_024 {
        futures_lite::io::copy(&mut seeking.reader_without_entry(index).await.unwrap(), &mut futures_lite::io::sink())
            .await
            .unwrap();
    }
    let source = seeking.into_inner().into_inner();
    assert_eq!(source.seeks, 1_024, "entry validation added seeks beyond opening each entry");
    let reader = ZipFileReader::new(data).await.unwrap();
    assert_eq!(reader.file().entries().len(), 1_024);
    // Independent readers must validate correctly even when entries finish in a different order.
    let mut tasks = Vec::new();
    for index in (0..1_024).rev() {
        let reader = reader.clone();
        tasks.push(tokio::spawn(async move {
            futures_lite::io::copy(&mut reader.reader_without_entry(index).await?, &mut futures_lite::io::sink())
                .await?;
            crate::error::Result::Ok(())
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
}

#[tokio::test]
async fn test_directory_range_must_fit_before_end_record() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;

    let mut data = b"PK\x05\x06".to_vec();
    data.resize(22, 0);
    // A directory byte at offset zero would overlap the EOCD record there.
    data[12] = 1;

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected invalid central directory range");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectoryRange { start: 0, end: 1, boundary: 0 }));
}

#[tokio::test]
async fn test_bound_directory_span_must_be_fully_parsed() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::spec::consts::{CDH_SIGNATURE, EOCDR_SIGNATURE};
    use crate::{Compression, ZipEntryBuilder};

    let mut inner = Vec::new();
    let mut writer = ZipFileWriter::new(&mut inner);
    writer.write_entry_whole(ZipEntryBuilder::new("inner".into(), Compression::Stored), b"B").await.unwrap();
    writer.close().await.unwrap();

    let mut data = inner.clone();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("outer".into(), Compression::Stored), b"A").await.unwrap();
    writer.close().await.unwrap();

    let inner_cd = data.windows(4).position(|window| window == CDH_SIGNATURE.to_le_bytes()).unwrap();
    let selected_eocdr = data.windows(4).rposition(|window| window == EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    let directory_size = (selected_eocdr - inner_cd) as u32;
    data[selected_eocdr + 12..selected_eocdr + 16].copy_from_slice(&directory_size.to_le_bytes());
    data[selected_eocdr + 16..selected_eocdr + 20].copy_from_slice(&(inner_cd as u32).to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected unparsed bytes in a bound directory span to fail");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectorySize { expected, actual } if expected > actual));
}

#[tokio::test]
async fn test_directory_allows_digital_signature_record() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::spec::consts::{CDDS_SIGNATURE, EOCDR_SIGNATURE};
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("signed".into(), Compression::Stored), b"A").await.unwrap();
    writer.close().await.unwrap();

    let eocdr = data.windows(4).position(|window| window == EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    let signature_data = b"signature";
    let mut signature_record = CDDS_SIGNATURE.to_le_bytes().to_vec();
    signature_record.extend_from_slice(&(signature_data.len() as u16).to_le_bytes());
    signature_record.extend_from_slice(signature_data);
    let signature_record_len = signature_record.len();
    data.splice(eocdr..eocdr, signature_record);

    let eocdr = eocdr + signature_record_len;
    let directory_size = u32::from_le_bytes(data[eocdr + 12..eocdr + 16].try_into().unwrap());
    data[eocdr + 12..eocdr + 16].copy_from_slice(&(directory_size + signature_record_len as u32).to_le_bytes());

    let reader = ZipFileReader::new(data).await.unwrap();
    assert_eq!(reader.file().entries().len(), 1);
    let mut entry = reader.reader_without_entry(0).await.unwrap();
    futures_lite::io::copy(&mut entry, &mut futures_lite::io::sink()).await.unwrap();
}

#[tokio::test]
async fn test_digital_signature_must_fill_declared_directory_span() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::spec::consts::{CDDS_SIGNATURE, EOCDR_SIGNATURE};
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("signed".into(), Compression::Stored), b"A").await.unwrap();
    writer.close().await.unwrap();

    let eocdr = data.windows(4).position(|window| window == EOCDR_SIGNATURE.to_le_bytes()).unwrap();
    let mut signature_record = CDDS_SIGNATURE.to_le_bytes().to_vec();
    signature_record.extend_from_slice(&2_u16.to_le_bytes());
    signature_record.extend_from_slice(b"x");
    let signature_record_len = signature_record.len();
    data.splice(eocdr..eocdr, signature_record);

    let eocdr = eocdr + signature_record_len;
    let directory_size = u32::from_le_bytes(data[eocdr + 12..eocdr + 16].try_into().unwrap());
    data[eocdr + 12..eocdr + 16].copy_from_slice(&(directory_size + signature_record_len as u32).to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected an incorrectly sized digital signature to fail");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectorySize { expected, actual } if actual > expected));
}

#[tokio::test]
async fn test_zip64_range_boundary_must_be_an_adjacent_end_record() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::spec::consts::{EOCDR_SIGNATURE, ZIP64_EOCDL_SIGNATURE, ZIP64_EOCDR_SIGNATURE};
    use crate::spec::header::{
        EndOfCentralDirectoryHeader, Zip64EndOfCentralDirectoryLocator, Zip64EndOfCentralDirectoryRecord,
    };
    use crate::{Compression, ZipEntryBuilder};

    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("visible".into(), Compression::Stored), b"A").await.unwrap();
    writer.close().await.unwrap();

    let zip64_eocdr_offset = data.len() as u64;
    data.extend_from_slice(&ZIP64_EOCDR_SIGNATURE.to_le_bytes());
    data.extend_from_slice(
        &Zip64EndOfCentralDirectoryRecord {
            size_of_zip64_end_of_cd_record: 44,
            version_made_by: 45,
            version_needed_to_extract: 45,
            disk_number: 0,
            disk_number_start_of_cd: 0,
            num_entries_in_directory_on_disk: 0,
            num_entries_in_directory: 0,
            directory_size: 0,
            offset_of_start_of_directory: zip64_eocdr_offset,
        }
        .as_bytes(),
    );
    data.extend_from_slice(b"gap!");
    data.extend_from_slice(&ZIP64_EOCDL_SIGNATURE.to_le_bytes());
    data.extend_from_slice(
        &Zip64EndOfCentralDirectoryLocator {
            number_of_disk_with_start_of_zip64_end_of_central_directory: 0,
            relative_offset: zip64_eocdr_offset,
            total_number_of_disks: 1,
        }
        .as_bytes(),
    );
    data.extend_from_slice(&EOCDR_SIGNATURE.to_le_bytes());
    data.extend_from_slice(
        &EndOfCentralDirectoryHeader {
            disk_num: u16::MAX,
            start_cent_dir_disk: u16::MAX,
            num_of_entries_disk: u16::MAX,
            num_of_entries: u16::MAX,
            size_cent_dir: u32::MAX,
            cent_dir_offset: u32::MAX,
            file_comm_length: 0,
        }
        .as_slice(),
    );

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected non-adjacent zip64 end record to fail");
    };
    assert!(matches!(err, ZipError::InvalidZip64EndOfCentralDirectoryLocatorOffset(..)));
}

#[tokio::test]
async fn test_zip64_locator_requires_end_record_signature() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;
    use crate::spec::consts::ZIP64_EOCDR_SIGNATURE;

    let mut data = include_bytes!("../zip64/zip64.zip").to_vec();
    let offset = data
        .windows(4)
        .position(|window| window == ZIP64_EOCDR_SIGNATURE.to_le_bytes())
        .expect("expected ZIP64 EOCD record");
    data[offset..offset + 4].copy_from_slice(&0_u32.to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected invalid ZIP64 end record signature to fail");
    };
    assert!(matches!(err, ZipError::UnexpectedHeaderError(0, ZIP64_EOCDR_SIGNATURE)));
}
#[tokio::test]
async fn test_zip64_end_record_size_must_cover_fixed_fields() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;
    use crate::spec::consts::ZIP64_EOCDR_SIGNATURE;

    let mut data = include_bytes!("../zip64/zip64.zip").to_vec();
    let offset = data
        .windows(4)
        .position(|window| window == ZIP64_EOCDR_SIGNATURE.to_le_bytes())
        .expect("expected ZIP64 EOCD record");
    data[offset + 4..offset + 12].copy_from_slice(&43_u64.to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected undersized ZIP64 end record to fail");
    };
    assert!(matches!(err, ZipError::InvalidZip64EndOfCentralDirectorySize(43)));
}

#[tokio::test]
async fn test_directory_must_bind_to_selected_end_record() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::{Compression, ZipEntryBuilder};

    let mut inner = Vec::new();
    let mut writer = ZipFileWriter::new(&mut inner);
    writer.write_entry_whole(ZipEntryBuilder::new("inner".into(), Compression::Stored), b"B").await.unwrap();
    writer.close().await.unwrap();

    let mut data = inner.clone();
    let mut writer = ZipFileWriter::new(&mut data);
    writer.write_entry_whole(ZipEntryBuilder::new("outer".into(), Compression::Stored), b"A").await.unwrap();
    writer.close().await.unwrap();

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected ambiguous central directory binding to fail");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectoryBinding { .. }));
}

#[tokio::test]
async fn repro_conflicting_zip64_and_legacy_central_directories() {
    use crate::base::read::mem::ZipFileReader;
    use crate::base::write::ZipFileWriter;
    use crate::error::ZipError;
    use crate::spec::consts::{EOCDR_SIGNATURE, ZIP64_EOCDL_SIGNATURE, ZIP64_EOCDR_SIGNATURE};
    use crate::spec::header::{Zip64EndOfCentralDirectoryLocator, Zip64EndOfCentralDirectoryRecord};
    use crate::{Compression, ZipEntryBuilder};

    // First create an ordinary archive whose legacy EOCD describes one entry.
    let mut data = Vec::new();
    let mut writer = ZipFileWriter::new(&mut data);
    writer
        .write_entry_whole(ZipEntryBuilder::new("visible.txt".into(), Compression::Stored), b"visible")
        .await
        .unwrap();
    writer.close().await.unwrap();

    let legacy_eocdr_offset =
        data.windows(4).rposition(|window| window == EOCDR_SIGNATURE.to_le_bytes()).expect("legacy EOCD record");
    let zip64_eocdr_offset = legacy_eocdr_offset as u64;

    // Insert a valid ZIP64 end record immediately before that EOCD, but make it describe an
    // empty directory. Both the legacy and ZIP64 directory spans end at this record, so the new
    // binding check alone cannot distinguish them.
    let mut zip64_trailer = ZIP64_EOCDR_SIGNATURE.to_le_bytes().to_vec();
    zip64_trailer.extend_from_slice(
        &Zip64EndOfCentralDirectoryRecord {
            size_of_zip64_end_of_cd_record: 44,
            version_made_by: 45,
            version_needed_to_extract: 45,
            disk_number: 0,
            disk_number_start_of_cd: 0,
            num_entries_in_directory_on_disk: 0,
            num_entries_in_directory: 0,
            directory_size: 0,
            offset_of_start_of_directory: zip64_eocdr_offset,
        }
        .as_bytes(),
    );
    zip64_trailer.extend_from_slice(&ZIP64_EOCDL_SIGNATURE.to_le_bytes());
    zip64_trailer.extend_from_slice(
        &Zip64EndOfCentralDirectoryLocator {
            number_of_disk_with_start_of_zip64_end_of_central_directory: 0,
            relative_offset: zip64_eocdr_offset,
            total_number_of_disks: 1,
        }
        .as_bytes(),
    );
    data.splice(legacy_eocdr_offset..legacy_eocdr_offset, zip64_trailer);

    // A unique-directory parser must reject the disagreement instead of choosing one record's
    // view of the archive.
    let Err(err) = ZipFileReader::new(data).await else {
        panic!("conflicting ZIP64 and legacy central-directory metadata was accepted");
    };
    assert!(matches!(err, ZipError::MismatchedZip64EndOfCentralDirectoryField { .. }));
}

#[tokio::test]
async fn test_each_concrete_legacy_end_field_must_match_zip64() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;
    use crate::spec::consts::ZIP64_EOCDR_SIGNATURE;

    // Offsets are relative to the start of the ZIP64 end-record signature. Each replacement
    // differs from the concrete value in the legacy EOCD while remaining cheap to parse.
    let mismatches = [
        ("disk number", 16, 4, 1_u64),
        ("central directory start disk", 20, 4, 1),
        ("number of entries on this disk", 24, 8, 0),
        ("number of entries", 32, 8, 0),
        ("central directory size", 40, 8, 0),
        ("central directory offset", 48, 8, 0),
    ];

    for (expected_field, field_offset, field_length, value) in mismatches {
        let mut data = include_bytes!("../zip64/zip64.zip").to_vec();
        let record_offset = data
            .windows(4)
            .position(|window| window == ZIP64_EOCDR_SIGNATURE.to_le_bytes())
            .expect("ZIP64 EOCD record");
        data[record_offset + field_offset..record_offset + field_offset + field_length]
            .copy_from_slice(&value.to_le_bytes()[..field_length]);

        let Err(err) = ZipFileReader::new(data).await else {
            panic!("mismatched {expected_field} was accepted");
        };
        assert!(
            matches!(err, ZipError::MismatchedZip64EndOfCentralDirectoryField { field, .. } if field == expected_field),
            "unexpected error for {expected_field}: {err:?}"
        );
    }
}
