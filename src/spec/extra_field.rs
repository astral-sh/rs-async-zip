// Copyright Cognite AS, 2023

use crate::error::{Result as ZipResult, ZipError};
use crate::spec::header::{
    ExtraField, HeaderId, InfoZipUnicodeCommentExtraField, InfoZipUnicodePathExtraField, UnknownExtraField,
    Zip64ExtendedInformationExtraField,
};

use super::consts::NON_ZIP64_MAX_SIZE;

/// Parse a zip64 extra field from bytes.
/// The content of "data" should exclude the header.
fn zip64_extended_information_field_from_bytes(
    _header_id: HeaderId,
    data: &[u8],
    header_uncompressed_size: u32,
    header_compressed_size: u32,
    header_relative_header_offset: Option<u32>,
    header_disk_start_number: Option<u16>,
) -> ZipResult<Zip64ExtendedInformationExtraField> {
    // slice.take is nightly-only so we'll just use an index to track the current position
    let mut current_idx = 0;
    let uncompressed_size = if header_uncompressed_size == NON_ZIP64_MAX_SIZE && data.len() >= current_idx + 8 {
        let val = Some(u64::from_le_bytes(data[current_idx..current_idx + 8].try_into().unwrap()));
        current_idx += 8;
        val
    } else {
        None
    };

    let compressed_size = if header_compressed_size == NON_ZIP64_MAX_SIZE && data.len() >= current_idx + 8 {
        let val = Some(u64::from_le_bytes(data[current_idx..current_idx + 8].try_into().unwrap()));
        current_idx += 8;
        val
    } else {
        None
    };

    let relative_header_offset =
        if header_relative_header_offset == Some(NON_ZIP64_MAX_SIZE) && data.len() >= current_idx + 8 {
            let val = Some(u64::from_le_bytes(data[current_idx..current_idx + 8].try_into().unwrap()));
            current_idx += 8;
            val
        } else {
            None
        };

    #[allow(unused_assignments)]
    let disk_start_number = if header_disk_start_number == Some(0xFFFF) && data.len() >= current_idx + 4 {
        let val = Some(u32::from_le_bytes(data[current_idx..current_idx + 4].try_into().unwrap()));
        current_idx += 4;
        val
    } else {
        None
    };

    if current_idx != data.len() {
        // In some cases, we've seen zips that include the zip64 extended information field with
        // uncompressed and compressed sizes equal to the local header sizes. We accept these, even
        // though they are not strictly compliant with the spec.
        if current_idx == 0 && data.len() == 16 {
            let uncompressed_size = u64::from_le_bytes(data[current_idx..current_idx + 8].try_into().unwrap());
            let compressed_size = u64::from_le_bytes(data[current_idx + 8..current_idx + 16].try_into().unwrap());
            if uncompressed_size == header_uncompressed_size as u64
                && compressed_size == header_compressed_size as u64
                && header_relative_header_offset.is_none()
                && header_disk_start_number.is_none()
            {
                return Ok(Zip64ExtendedInformationExtraField {
                    uncompressed_size: Some(uncompressed_size),
                    compressed_size: Some(compressed_size),
                    relative_header_offset: None,
                    disk_start_number: None,
                });
            }
        }

        return Err(ZipError::Zip64ExtendedInformationFieldTooLong { expected: data.len(), actual: current_idx });
    }

    Ok(Zip64ExtendedInformationExtraField {
        uncompressed_size,
        compressed_size,
        relative_header_offset,
        disk_start_number,
    })
}

fn info_zip_unicode_comment_extra_field_from_bytes(
    _header_id: HeaderId,
    data_size: u16,
    data: &[u8],
) -> ZipResult<InfoZipUnicodeCommentExtraField> {
    if data.is_empty() {
        return Err(ZipError::InfoZipUnicodeCommentFieldIncomplete);
    }
    let version = data[0];
    match version {
        1 => {
            if data.len() < 5 {
                return Err(ZipError::InfoZipUnicodeCommentFieldIncomplete);
            }
            let crc32 = u32::from_le_bytes(data[1..5].try_into().unwrap());
            let unicode = data[5..(data_size as usize)].to_vec();
            Ok(InfoZipUnicodeCommentExtraField::V1 { crc32, unicode })
        }
        _ => Ok(InfoZipUnicodeCommentExtraField::Unknown { version, data: data[1..(data_size as usize)].to_vec() }),
    }
}

fn info_zip_unicode_path_extra_field_from_bytes(
    _header_id: HeaderId,
    data_size: u16,
    data: &[u8],
) -> ZipResult<InfoZipUnicodePathExtraField> {
    if data.is_empty() {
        return Err(ZipError::InfoZipUnicodePathFieldIncomplete);
    }
    let version = data[0];
    match version {
        1 => {
            if data.len() < 5 {
                return Err(ZipError::InfoZipUnicodePathFieldIncomplete);
            }
            let crc32 = u32::from_le_bytes(data[1..5].try_into().unwrap());
            let unicode = data[5..(data_size as usize)].to_vec();
            Ok(InfoZipUnicodePathExtraField::V1 { crc32, unicode })
        }
        _ => Ok(InfoZipUnicodePathExtraField::Unknown { version, data: data[1..(data_size as usize)].to_vec() }),
    }
}

pub(crate) fn extra_field_from_bytes(
    header_id: HeaderId,
    data_size: u16,
    data: &[u8],
    uncompressed_size: u32,
    compressed_size: u32,
    relative_header_offset: Option<u32>,
    disk_start_number: Option<u16>,
) -> ZipResult<ExtraField> {
    match header_id {
        HeaderId::ZIP64_EXTENDED_INFORMATION_EXTRA_FIELD => {
            Ok(ExtraField::Zip64ExtendedInformation(zip64_extended_information_field_from_bytes(
                header_id,
                data,
                uncompressed_size,
                compressed_size,
                relative_header_offset,
                disk_start_number,
            )?))
        }
        HeaderId::INFO_ZIP_UNICODE_COMMENT_EXTRA_FIELD => Ok(ExtraField::InfoZipUnicodeComment(
            info_zip_unicode_comment_extra_field_from_bytes(header_id, data_size, data)?,
        )),
        HeaderId::INFO_ZIP_UNICODE_PATH_EXTRA_FIELD => Ok(ExtraField::InfoZipUnicodePath(
            info_zip_unicode_path_extra_field_from_bytes(header_id, data_size, data)?,
        )),
        _ => Ok(ExtraField::Unknown(UnknownExtraField { header_id, data_size, content: data.to_vec() })),
    }
}
