// Copyright (c) 2023 Harry [Majored] [hello@majored.pw]
// Copyright (c) 2023 Cognite AS
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use futures_lite::io::AsyncReadExt;

use crate::tests::init_logger;

const ZIP64_ZIP_CONTENTS: &str = "Hello World!\n";

/// Tests opening and reading a zip64 archive.
/// It contains one file named "-" with a zip 64 extended field header.
#[tokio::test]
async fn test_read_zip64_archive_mem() {
    use crate::base::read::mem::ZipFileReader;
    init_logger();

    let data = include_bytes!("zip64.zip").to_vec();

    let reader = ZipFileReader::new(data).await.unwrap();
    let mut entry_reader = reader.reader_without_entry(0).await.unwrap();

    let mut read_data = String::new();
    entry_reader.read_to_string(&mut read_data).await.expect("read failed");

    assert_eq!(
        read_data.chars().count(),
        ZIP64_ZIP_CONTENTS.chars().count(),
        "{read_data:?} != {ZIP64_ZIP_CONTENTS:?}"
    );
    assert_eq!(read_data, ZIP64_ZIP_CONTENTS);
}

/// Like test_read_zip64_archive_mem() but for the streaming version
#[tokio::test]
async fn test_read_zip64_archive_stream() {
    use crate::base::read::stream::ZipFileReader;
    init_logger();

    let data = include_bytes!("zip64.zip").to_vec();

    let reader = ZipFileReader::new(data.as_slice());
    let mut entry_reader = reader.next_without_entry().await.unwrap().unwrap();

    let mut read_data = String::new();
    entry_reader.reader_mut().read_to_string(&mut read_data).await.expect("read failed");

    assert_eq!(
        read_data.chars().count(),
        ZIP64_ZIP_CONTENTS.chars().count(),
        "{read_data:?} != {ZIP64_ZIP_CONTENTS:?}"
    );
    assert_eq!(read_data, ZIP64_ZIP_CONTENTS);
}

#[tokio::test]
async fn test_zip64_entry_count_must_fit_before_zip64_eocdr() {
    use crate::base::read::mem::ZipFileReader;
    use crate::error::ZipError;
    use crate::spec::consts::{EOCDR_SIGNATURE, ZIP64_EOCDL_SIGNATURE, ZIP64_EOCDR_SIGNATURE};
    use crate::spec::header::Zip64EndOfCentralDirectoryRecord;

    let mut data = Vec::new();
    data.extend_from_slice(&ZIP64_EOCDR_SIGNATURE.to_le_bytes());
    data.extend_from_slice(
        &Zip64EndOfCentralDirectoryRecord {
            size_of_zip64_end_of_cd_record: 44,
            version_made_by: 45,
            version_needed_to_extract: 45,
            disk_number: 0,
            disk_number_start_of_cd: 0,
            num_entries_in_directory_on_disk: u64::MAX,
            num_entries_in_directory: u64::MAX,
            directory_size: 0,
            offset_of_start_of_directory: 0,
        }
        .as_bytes(),
    );
    data.extend_from_slice(&ZIP64_EOCDL_SIGNATURE.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&0_u64.to_le_bytes());
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&EOCDR_SIGNATURE.to_le_bytes());
    data.extend_from_slice(&0_u16.to_le_bytes());
    data.extend_from_slice(&0_u16.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&0_u16.to_le_bytes());

    let Err(err) = ZipFileReader::new(data).await else {
        panic!("expected invalid central directory entry count");
    };
    assert!(matches!(err, ZipError::InvalidCentralDirectoryEntryCount { entries: u64::MAX }));
}

/// Generate an example file only if it doesn't exist already.
/// The file is placed adjacent to this rs file.
#[cfg(feature = "tokio")]
fn generate_zip64many_zip() -> std::path::PathBuf {
    use std::io::Write;
    use zip::write::{ExtendedFileOptions, FileOptions};

    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/tests/read/zip64/zip64many.zip");

    // Only recreate the zip if it doesnt already exist.
    if path.exists() {
        return path;
    }

    let zip_file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(zip_file);
    let options: FileOptions<'_, ExtendedFileOptions> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for i in 0..2_u32.pow(16) + 1 {
        zip.start_file(format!("{i}.txt"), options.clone()).unwrap();
        zip.write_all(b"\n").unwrap();
    }

    zip.finish().unwrap();

    path
}

/// Test reading a generated zip64 archive that contains more than 2^16 entries.
#[cfg(feature = "tokio-fs")]
#[tokio::test]
async fn test_read_zip64_archive_many_entries() {
    use crate::tokio::read::fs::ZipFileReader;

    init_logger();

    let path = generate_zip64many_zip();

    let reader = ZipFileReader::new(path).await.unwrap();

    // Verify that each entry exists and is has the contents "\n"
    for i in 0..2_u32.pow(16) + 1 {
        let entry = reader.file().entries().get(i as usize).unwrap();
        eprintln!("{:?}", entry.filename().as_bytes());
        assert_eq!(entry.filename.as_str().unwrap(), format!("{i}.txt"));
        let mut entry = reader.reader_without_entry(i as usize).await.unwrap();
        let mut contents = String::new();
        entry.read_to_string(&mut contents).await.unwrap();
        assert_eq!(contents, "\n");
    }
}
