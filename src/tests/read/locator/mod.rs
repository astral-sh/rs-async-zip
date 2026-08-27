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
async fn locator_accepts_maximum_comment_and_padding_with_short_reads() {
    use crate::base::read::seek::ZipFileReader;
    use futures_lite::io::{BufReader, Cursor};

    for nul_comment in [false, true] {
        let mut data = include_bytes!("empty-with-max-comment.zip").to_vec();
        if nul_comment {
            // Declared comment bytes must not count toward the padding limit, even if all NUL.
            data[22..].fill(0);
        }
        data.resize(data.len() + 4096, 0);
        let reader = BufReader::new(super::ShortReader::new(Cursor::new(data), 3));
        ZipFileReader::new(reader).await.unwrap();
    }
}

#[tokio::test]
async fn locator_rejects_all_zero_input() {
    use crate::base::read::io::locator::eocdr;
    use crate::error::ZipError;
    use futures_lite::io::Cursor;

    let mut data = include_bytes!("empty.zip").to_vec();
    data.fill(0);
    data.resize(2 * 1024 * 1024, 0);
    let mut reader = super::ShortReader::new(Cursor::new(data), 2048);
    assert!(matches!(eocdr(&mut reader).await, Err(ZipError::UnableToLocateEOCDR)));
    assert!(reader.bytes_read < 72 * 1024, "EOCD lookup read {} bytes", reader.bytes_read);
}
