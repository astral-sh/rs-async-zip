// Copyright (c) 2023 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use tokio::io::BufReader;
use tokio_util::compat::TokioAsyncReadCompatExt;

mod common;

#[cfg(feature = "zstd")]
const ZSTD_ZIP_FILE: &str = "tests/test_inputs/sample_data.zstd.zip";
#[cfg(feature = "deflate")]
const DEFLATE_ZIP_FILE: &str = "tests/test_inputs/sample_data.deflate.zip";
const STORE_ZIP_FILE: &str = "tests/test_inputs/sample_data.store.zip";
const UTF8_EXTRA_ZIP_FILE: &str = "tests/test_inputs/sample_data_utf8_extra.zip";

#[cfg(feature = "zstd")]
#[tokio::test]
async fn decompress_zstd_zip_seek() {
    common::check_decompress_seek(ZSTD_ZIP_FILE).await
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn decompress_deflate_zip_seek() {
    common::check_decompress_seek(DEFLATE_ZIP_FILE).await
}

#[tokio::test]
async fn decompress_store_zip_seek() {
    common::check_decompress_seek(STORE_ZIP_FILE).await
}

#[cfg(feature = "zstd")]
#[tokio::test]
async fn decompress_zstd_zip_mem() {
    let content = tokio::fs::read(ZSTD_ZIP_FILE).await.unwrap();
    common::check_decompress_mem(content).await
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn decompress_deflate_zip_mem() {
    let content = tokio::fs::read(DEFLATE_ZIP_FILE).await.unwrap();
    common::check_decompress_mem(content).await
}

#[tokio::test]
async fn decompress_store_zip_mem() {
    let content = tokio::fs::read(STORE_ZIP_FILE).await.unwrap();
    common::check_decompress_mem(content).await
}

#[cfg(feature = "zstd")]
#[cfg(feature = "tokio-fs")]
#[tokio::test]
async fn decompress_zstd_zip_fs() {
    common::check_decompress_fs(ZSTD_ZIP_FILE).await
}

#[cfg(feature = "deflate")]
#[cfg(feature = "tokio-fs")]
#[tokio::test]
async fn decompress_deflate_zip_fs() {
    common::check_decompress_fs(DEFLATE_ZIP_FILE).await
}

#[cfg(feature = "tokio-fs")]
#[tokio::test]
async fn decompress_store_zip_fs() {
    common::check_decompress_fs(STORE_ZIP_FILE).await
}

#[tokio::test]
async fn decompress_zip_with_utf8_extra() {
    let file = BufReader::new(tokio::fs::File::open(UTF8_EXTRA_ZIP_FILE).await.unwrap());
    let mut file_compat = file.compat();
    let zip = async_zip::base::read::seek::ZipFileReader::new(&mut file_compat).await.unwrap();
    let zip_entries: Vec<_> = zip.file().entries().to_vec();
    assert_eq!(zip_entries.len(), 1);
    assert_eq!(zip_entries[0].header_size(), 93);
    assert_eq!(zip_entries[0].filename().as_str().unwrap(), "\u{4E2D}\u{6587}.txt");
    assert_eq!(zip_entries[0].filename().alternative(), Some(b"\xD6\xD0\xCe\xC4.txt".as_ref()));
}

/// Build a minimal in-memory ZIP whose central directory entries include a zip64 extended
/// information extra field even though all sizes fit in 32 bits (no sentinel values).
///
/// This is exactly what Python's `zipfile` module (and conda-build) produces. Before the fix,
/// parsing such a ZIP with the seek-based or mem-based reader would fail with
/// `Zip64ExtendedInformationFieldTooLong`.
fn build_zip_with_cd_redundant_zip64_extra() -> Vec<u8> {
    let entries: &[(&str, &[u8])] = &[("a.txt", b"aaaaa"), ("b.txt", b"bbbbb"), ("c.txt", b"ccccc")];

    let mut buf: Vec<u8> = Vec::new();
    let mut lh_offsets: Vec<u32> = Vec::new();

    // ---- Local file headers + file data ----
    for (name, data) in entries {
        lh_offsets.push(buf.len() as u32);
        let crc = crc32fast::hash(data);
        let size32 = data.len() as u32;
        let size64 = data.len() as u64;

        buf.extend_from_slice(&0x04034b50_u32.to_le_bytes()); // LFH signature
        buf.extend_from_slice(&45_u16.to_le_bytes()); // version needed (zip64)
        buf.extend_from_slice(&0_u16.to_le_bytes()); // flags
        buf.extend_from_slice(&0_u16.to_le_bytes()); // compression (stored)
        buf.extend_from_slice(&0_u16.to_le_bytes()); // mod time
        buf.extend_from_slice(&0_u16.to_le_bytes()); // mod date
        buf.extend_from_slice(&crc.to_le_bytes()); // crc32
        buf.extend_from_slice(&size32.to_le_bytes()); // compressed size (actual, not sentinel)
        buf.extend_from_slice(&size32.to_le_bytes()); // uncompressed size (actual, not sentinel)
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes()); // filename length
        buf.extend_from_slice(&20_u16.to_le_bytes()); // extra field length (4 hdr + 16 data)
        buf.extend_from_slice(name.as_bytes()); // filename
                                                // zip64 extra field
        buf.extend_from_slice(&0x0001_u16.to_le_bytes()); // header id
        buf.extend_from_slice(&16_u16.to_le_bytes()); // data size
        buf.extend_from_slice(&size64.to_le_bytes()); // uncompressed size as u64
        buf.extend_from_slice(&size64.to_le_bytes()); // compressed size as u64
        buf.extend_from_slice(data); // file data
    }

    let cd_offset = buf.len() as u32;

    // ---- Central directory ----
    for (i, (name, data)) in entries.iter().enumerate() {
        let crc = crc32fast::hash(data);
        let size32 = data.len() as u32;
        let size64 = data.len() as u64;

        buf.extend_from_slice(&0x02014b50_u32.to_le_bytes()); // CDH signature
        buf.extend_from_slice(&0_u16.to_le_bytes()); // version made by
        buf.extend_from_slice(&45_u16.to_le_bytes()); // version needed
        buf.extend_from_slice(&0_u16.to_le_bytes()); // flags
        buf.extend_from_slice(&0_u16.to_le_bytes()); // compression
        buf.extend_from_slice(&0_u16.to_le_bytes()); // mod time
        buf.extend_from_slice(&0_u16.to_le_bytes()); // mod date
        buf.extend_from_slice(&crc.to_le_bytes()); // crc32
        buf.extend_from_slice(&size32.to_le_bytes()); // compressed size (actual, not sentinel)
        buf.extend_from_slice(&size32.to_le_bytes()); // uncompressed size (actual, not sentinel)
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes()); // filename length
        buf.extend_from_slice(&20_u16.to_le_bytes()); // extra field length
        buf.extend_from_slice(&0_u16.to_le_bytes()); // comment length
        buf.extend_from_slice(&0_u16.to_le_bytes()); // disk start (Some(0) in parse)
        buf.extend_from_slice(&0_u16.to_le_bytes()); // internal attributes
        buf.extend_from_slice(&0_u32.to_le_bytes()); // external attributes
        buf.extend_from_slice(&lh_offsets[i].to_le_bytes()); // LFH offset (Some(x) in parse)
        buf.extend_from_slice(name.as_bytes()); // filename
                                                // zip64 extra field — same redundant format as in the LFH
        buf.extend_from_slice(&0x0001_u16.to_le_bytes()); // header id
        buf.extend_from_slice(&16_u16.to_le_bytes()); // data size
        buf.extend_from_slice(&size64.to_le_bytes()); // uncompressed size as u64
        buf.extend_from_slice(&size64.to_le_bytes()); // compressed size as u64
    }

    let cd_size = buf.len() as u32 - cd_offset;

    // ---- End of central directory record ----
    buf.extend_from_slice(&0x06054b50_u32.to_le_bytes()); // EOCDR signature
    buf.extend_from_slice(&0_u16.to_le_bytes()); // disk number
    buf.extend_from_slice(&0_u16.to_le_bytes()); // start disk
    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // entries on this disk
    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // total entries
    buf.extend_from_slice(&cd_size.to_le_bytes()); // CD size
    buf.extend_from_slice(&cd_offset.to_le_bytes()); // CD offset
    buf.extend_from_slice(&0_u16.to_le_bytes()); // comment length

    buf
}

/// Regression test for conda-build / Python zipfile-style archives: central directory entries
/// carry a zip64 extra field with redundant (non-sentinel) sizes. Before the fix this returned
/// `Zip64ExtendedInformationFieldTooLong`.
#[tokio::test]
async fn cd_zip64_extra_with_non_sentinel_sizes_is_readable() {
    let zip_data = build_zip_with_cd_redundant_zip64_extra();
    let zip = async_zip::base::read::mem::ZipFileReader::new(zip_data)
        .await
        .expect("reading a ZIP with redundant zip64 extra fields in the central directory should succeed");

    let expected: &[(&str, &[u8])] = &[("a.txt", b"aaaaa"), ("b.txt", b"bbbbb"), ("c.txt", b"ccccc")];
    assert_eq!(zip.file().entries().len(), expected.len());

    for (idx, (name, content)) in expected.iter().enumerate() {
        let entry = &zip.file().entries()[idx];
        assert_eq!(entry.filename().as_str().unwrap(), *name);
        assert_eq!(entry.uncompressed_size(), 5);
        assert_eq!(entry.compressed_size(), 5);

        let mut buf = Vec::new();
        let mut reader = zip.reader_with_entry(idx).await.unwrap();
        reader.read_to_end_checked(&mut buf).await.expect("CRC check should pass");
        assert_eq!(buf.as_slice(), *content);
    }
}
