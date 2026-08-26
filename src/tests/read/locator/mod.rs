// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

#[test]
fn search_one_byte_test() {
    let buffer: &[u8] = &[0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_none());

    let buffer: &[u8] = &[0x2, 0x1, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_some());
    assert_eq!(1, matched.unwrap());
}

#[test]
fn search_two_byte_test() {
    let buffer: &[u8] = &[0x2, 0x1, 0x0, 0x0, 0x0, 0x0];
    let signature: &[u8] = &[0x2, 0x1];

    let matched = crate::base::read::io::locator::reverse_search_buffer(buffer, signature);
    assert!(matched.is_some());
    assert_eq!(1, matched.unwrap());
}

#[tokio::test]
async fn locator_empty_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_short_reads_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty.zip");
    let mut reader = super::ShortReader::new(Cursor::new(data), 3);
    let eocdr = crate::base::read::io::locator::eocdr(&mut reader).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_empty_max_comment_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty-with-max-comment.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_buffer_boundary_test() {
    use futures_lite::io::Cursor;

    let data = &include_bytes!("empty-buffer-boundary.zip");
    let mut cursor = Cursor::new(data);
    let eocdr = crate::base::read::io::locator::eocdr(&mut cursor).await;

    assert!(eocdr.is_ok());
    assert_eq!(eocdr.unwrap(), 4);
}

#[tokio::test]
async fn locator_accepts_long_nul_padding() {
    use crate::base::read::{io::locator::eocdr, mem, seek};
    use futures_lite::io::Cursor;

    // Neither uv nor Malo has long-padding cases. Keep the existing archives intact and append
    // only padding beyond the maximum EOCD/comment search window, including chunk boundaries.
    let archives: &[&[u8]] = &[
        include_bytes!("empty.zip"),
        include_bytes!("empty-with-max-comment.zip"),
        include_bytes!("empty-buffer-boundary.zip"),
        include_bytes!("../malo/accept/store.zip"),
        include_bytes!("../malo/accept/comment.zip"),
    ];
    for &archive in archives {
        let expected = eocdr(Cursor::new(archive)).await.unwrap();
        for padding in [65_557, 65_558, 67_606] {
            let mut data = archive.to_vec();
            data.resize(data.len() + padding, 0);
            assert_eq!(eocdr(Cursor::new(&data)).await.unwrap(), expected);
            seek::ZipFileReader::new(Cursor::new(&data)).await.unwrap();
            mem::ZipFileReader::new(data).await.unwrap();
        }
    }
}

#[cfg(feature = "deflate")]
#[tokio::test]
async fn locator_accepts_long_nul_padding_after_zip64() {
    use crate::base::read::{io::locator::eocdr, mem, seek};
    use futures_lite::io::Cursor;

    let mut data = include_bytes!("../malo/accept/zip64_eocd.zip").to_vec();
    let expected = eocdr(Cursor::new(&data)).await.unwrap();
    data.resize(data.len() + 131_072, 0);
    assert_eq!(eocdr(Cursor::new(&data)).await.unwrap(), expected);
    super::cd::read_streamed_archive(&data[..]).await.unwrap();
    seek::ZipFileReader::new(Cursor::new(&data)).await.unwrap();
    mem::ZipFileReader::new(data).await.unwrap();
}

#[tokio::test]
async fn locator_accepts_long_nul_padding_with_short_reads() {
    use crate::base::read::io::locator::eocdr;
    use futures_lite::io::Cursor;

    let mut data = include_bytes!("empty.zip").to_vec();
    data.resize(data.len() + 65_558, 0);
    let reader = super::ShortReader::new(Cursor::new(data), 3);
    assert_eq!(eocdr(reader).await.unwrap(), 4);
}

#[tokio::test]
async fn locator_rejects_all_zero_input() {
    use crate::base::read::io::locator::eocdr;
    use crate::error::ZipError;
    use futures_lite::io::Cursor;

    let mut data = include_bytes!("empty.zip").to_vec();
    data.fill(0);
    data.resize(65_558, 0);
    assert!(matches!(eocdr(Cursor::new(data)).await, Err(ZipError::UnableToLocateEOCDR)));
}

#[tokio::test]
async fn terminal_zero_fields_must_not_hide_truncation() {
    use crate::base::read::seek::ZipFileReader;
    use futures_lite::io::Cursor;

    // The zero-filled EOCD fields are required bytes, not optional NUL padding.
    let data = include_bytes!("empty.zip");
    let error = ZipFileReader::new(Cursor::new(&data[..data.len() - 1])).await.err().unwrap();
    super::assert_unexpected_eof(error);
}

#[tokio::test]
async fn nonzero_contents_after_long_padding_are_rejected() {
    use crate::base::read::seek::ZipFileReader;
    use futures_lite::io::Cursor;

    let mut data = include_bytes!("../malo/accept/store.zip").to_vec();
    data.resize(data.len() + 131_072, 0);
    data.push(b'X');
    assert!(ZipFileReader::new(Cursor::new(&data)).await.is_err());
    assert!(super::cd::read_streamed_archive(&data[..]).await.is_err());
}
