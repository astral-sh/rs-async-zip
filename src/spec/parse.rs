// Copyright (c) 2021 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::error::{Result, ZipError};
use crate::spec::header::{
    CentralDirectoryRecord, EndOfCentralDirectoryHeader, ExtraField, GeneralPurposeFlag, HeaderId, LocalFileHeader,
    Zip64EndOfCentralDirectoryLocator, Zip64EndOfCentralDirectoryRecord,
};

use futures_lite::io::{AsyncRead, AsyncReadExt};

impl From<[u8; 26]> for LocalFileHeader {
    fn from(value: [u8; 26]) -> LocalFileHeader {
        LocalFileHeader {
            version: u16::from_le_bytes(value[0..2].try_into().unwrap()),
            flags: GeneralPurposeFlag::from(u16::from_le_bytes(value[2..4].try_into().unwrap())),
            compression: u16::from_le_bytes(value[4..6].try_into().unwrap()),
            mod_time: u16::from_le_bytes(value[6..8].try_into().unwrap()),
            mod_date: u16::from_le_bytes(value[8..10].try_into().unwrap()),
            crc: u32::from_le_bytes(value[10..14].try_into().unwrap()),
            compressed_size: u32::from_le_bytes(value[14..18].try_into().unwrap()),
            uncompressed_size: u32::from_le_bytes(value[18..22].try_into().unwrap()),
            file_name_length: u16::from_le_bytes(value[22..24].try_into().unwrap()),
            extra_field_length: u16::from_le_bytes(value[24..26].try_into().unwrap()),
        }
    }
}

impl From<u16> for GeneralPurposeFlag {
    fn from(value: u16) -> GeneralPurposeFlag {
        let encrypted = !matches!(value & 0x1, 0);
        let data_descriptor = !matches!((value & 0x8) >> 3, 0);
        let filename_unicode = !matches!((value & 0x800) >> 11, 0);

        GeneralPurposeFlag { encrypted, data_descriptor, filename_unicode }
    }
}

impl From<[u8; 42]> for CentralDirectoryRecord {
    fn from(value: [u8; 42]) -> CentralDirectoryRecord {
        CentralDirectoryRecord {
            v_made_by: u16::from_le_bytes(value[0..2].try_into().unwrap()),
            v_needed: u16::from_le_bytes(value[2..4].try_into().unwrap()),
            flags: GeneralPurposeFlag::from(u16::from_le_bytes(value[4..6].try_into().unwrap())),
            compression: u16::from_le_bytes(value[6..8].try_into().unwrap()),
            mod_time: u16::from_le_bytes(value[8..10].try_into().unwrap()),
            mod_date: u16::from_le_bytes(value[10..12].try_into().unwrap()),
            crc: u32::from_le_bytes(value[12..16].try_into().unwrap()),
            compressed_size: u32::from_le_bytes(value[16..20].try_into().unwrap()),
            uncompressed_size: u32::from_le_bytes(value[20..24].try_into().unwrap()),
            file_name_length: u16::from_le_bytes(value[24..26].try_into().unwrap()),
            extra_field_length: u16::from_le_bytes(value[26..28].try_into().unwrap()),
            file_comment_length: u16::from_le_bytes(value[28..30].try_into().unwrap()),
            disk_start: u16::from_le_bytes(value[30..32].try_into().unwrap()),
            inter_attr: u16::from_le_bytes(value[32..34].try_into().unwrap()),
            exter_attr: u32::from_le_bytes(value[34..38].try_into().unwrap()),
            lh_offset: u32::from_le_bytes(value[38..42].try_into().unwrap()),
        }
    }
}

impl From<[u8; 18]> for EndOfCentralDirectoryHeader {
    fn from(value: [u8; 18]) -> EndOfCentralDirectoryHeader {
        EndOfCentralDirectoryHeader {
            disk_num: u16::from_le_bytes(value[0..2].try_into().unwrap()),
            start_cent_dir_disk: u16::from_le_bytes(value[2..4].try_into().unwrap()),
            num_of_entries_disk: u16::from_le_bytes(value[4..6].try_into().unwrap()),
            num_of_entries: u16::from_le_bytes(value[6..8].try_into().unwrap()),
            size_cent_dir: u32::from_le_bytes(value[8..12].try_into().unwrap()),
            cent_dir_offset: u32::from_le_bytes(value[12..16].try_into().unwrap()),
            file_comm_length: u16::from_le_bytes(value[16..18].try_into().unwrap()),
        }
    }
}

impl From<[u8; 52]> for Zip64EndOfCentralDirectoryRecord {
    fn from(value: [u8; 52]) -> Self {
        Self {
            size_of_zip64_end_of_cd_record: u64::from_le_bytes(value[0..8].try_into().unwrap()),
            version_made_by: u16::from_le_bytes(value[8..10].try_into().unwrap()),
            version_needed_to_extract: u16::from_le_bytes(value[10..12].try_into().unwrap()),
            disk_number: u32::from_le_bytes(value[12..16].try_into().unwrap()),
            disk_number_start_of_cd: u32::from_le_bytes(value[16..20].try_into().unwrap()),
            num_entries_in_directory_on_disk: u64::from_le_bytes(value[20..28].try_into().unwrap()),
            num_entries_in_directory: u64::from_le_bytes(value[28..36].try_into().unwrap()),
            directory_size: u64::from_le_bytes(value[36..44].try_into().unwrap()),
            offset_of_start_of_directory: u64::from_le_bytes(value[44..52].try_into().unwrap()),
        }
    }
}

impl From<[u8; 16]> for Zip64EndOfCentralDirectoryLocator {
    fn from(value: [u8; 16]) -> Self {
        Self {
            number_of_disk_with_start_of_zip64_end_of_central_directory: u32::from_le_bytes(
                value[0..4].try_into().unwrap(),
            ),
            relative_offset: u64::from_le_bytes(value[4..12].try_into().unwrap()),
            total_number_of_disks: u32::from_le_bytes(value[12..16].try_into().unwrap()),
        }
    }
}

impl From<[u8; 16]> for DataDescriptor {
    fn from(value: [u8; 16]) -> Self {
        Self {
            crc: u32::from_le_bytes(value[0..4].try_into().unwrap()),
            compressed_size: u32::from_le_bytes(value[4..8].try_into().unwrap()),
            uncompressed_size: u32::from_le_bytes(value[8..12].try_into().unwrap()),
        }
    }
}

impl LocalFileHeader {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<LocalFileHeader> {
        let mut buffer: [u8; 26] = [0; 26];
        reader.read_exact(&mut buffer).await?;
        Ok(LocalFileHeader::from(buffer))
    }
}

impl EndOfCentralDirectoryHeader {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<EndOfCentralDirectoryHeader> {
        let mut buffer: [u8; 18] = [0; 18];
        reader.read_exact(&mut buffer).await?;
        Ok(EndOfCentralDirectoryHeader::from(buffer))
    }
}

impl CentralDirectoryRecord {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<CentralDirectoryRecord> {
        let mut buffer: [u8; 42] = [0; 42];
        reader.read_exact(&mut buffer).await?;
        Ok(CentralDirectoryRecord::from(buffer))
    }
}

impl Zip64EndOfCentralDirectoryRecord {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Zip64EndOfCentralDirectoryRecord> {
        let mut buffer: [u8; 52] = [0; 52];
        reader.read_exact(&mut buffer).await?;
        Ok(Self::from(buffer))
    }

    pub fn as_bytes(&self) -> [u8; 52] {
        let mut array = [0; 52];
        let mut cursor = 0;

        array_push!(array, cursor, self.size_of_zip64_end_of_cd_record.to_le_bytes());
        array_push!(array, cursor, self.version_made_by.to_le_bytes());
        array_push!(array, cursor, self.version_needed_to_extract.to_le_bytes());
        array_push!(array, cursor, self.disk_number.to_le_bytes());
        array_push!(array, cursor, self.disk_number_start_of_cd.to_le_bytes());
        array_push!(array, cursor, self.num_entries_in_directory_on_disk.to_le_bytes());
        array_push!(array, cursor, self.num_entries_in_directory.to_le_bytes());
        array_push!(array, cursor, self.directory_size.to_le_bytes());
        array_push!(array, cursor, self.offset_of_start_of_directory.to_le_bytes());

        array
    }
}

impl DataDescriptor {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<DataDescriptor> {
        let mut descriptor: [u8; DATA_DESCRIPTOR_LENGTH] = [0; DATA_DESCRIPTOR_LENGTH];
        reader.read_exact(&mut descriptor).await?;

        // The data descriptor signature is optional.
        if descriptor[0..SIGNATURE_LENGTH] == DATA_DESCRIPTOR_SIGNATURE.to_le_bytes() {
            // If present, read the remaining bytes.
            let mut tail: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
            reader.read_exact(&mut tail).await?;

            Ok(DataDescriptor {
                crc: u32::from_le_bytes(descriptor[4..8].try_into().unwrap()),
                compressed_size: u32::from_le_bytes(descriptor[8..12].try_into().unwrap()),
                uncompressed_size: u32::from_le_bytes(tail[0..4].try_into().unwrap()),
            })
        } else {
            // If absent, then the first four bytes are not the signature, but instead part of the
            // data descriptor.
            Ok(DataDescriptor {
                crc: u32::from_le_bytes(descriptor[0..4].try_into().unwrap()),
                compressed_size: u32::from_le_bytes(descriptor[4..8].try_into().unwrap()),
                uncompressed_size: u32::from_le_bytes(descriptor[8..12].try_into().unwrap()),
            })
        }
    }

    pub fn as_bytes(&self) -> [u8; DATA_DESCRIPTOR_LENGTH] {
        let mut array = [0; DATA_DESCRIPTOR_LENGTH];
        let mut cursor = 0;

        array_push!(array, cursor, self.crc.to_le_bytes());
        array_push!(array, cursor, self.compressed_size.to_le_bytes());
        array_push!(array, cursor, self.uncompressed_size.to_le_bytes());

        array
    }
}

impl Zip64DataDescriptor {
    pub async fn from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Zip64DataDescriptor> {
        // Read the first four bytes to check for the data descriptor signature.
        let mut signature: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
        reader.read_exact(&mut signature).await?;

        // The data descriptor signature is optional.
        if signature[0..SIGNATURE_LENGTH] == DATA_DESCRIPTOR_SIGNATURE.to_le_bytes() {
            // If present, read the remaining bytes.
            let mut descriptor: [u8; ZIP64_DATA_DESCRIPTOR_LENGTH] = [0; ZIP64_DATA_DESCRIPTOR_LENGTH];
            reader.read_exact(&mut descriptor).await?;

            Ok(Zip64DataDescriptor {
                crc: u32::from_le_bytes(descriptor[0..4].try_into().unwrap()),
                compressed_size: u64::from_le_bytes(descriptor[4..12].try_into().unwrap()),
                uncompressed_size: u64::from_le_bytes(descriptor[12..20].try_into().unwrap()),
            })
        } else {
            // If absent, read the remaining bytes without the signature, and use the first four
            // bytes as the CRC.
            let mut descriptor: [u8; ZIP64_DATA_DESCRIPTOR_LENGTH - SIGNATURE_LENGTH] =
                [0; ZIP64_DATA_DESCRIPTOR_LENGTH - SIGNATURE_LENGTH];
            reader.read_exact(&mut descriptor).await?;

            Ok(Zip64DataDescriptor {
                crc: u32::from_le_bytes(signature),
                compressed_size: u64::from_le_bytes(descriptor[0..8].try_into().unwrap()),
                uncompressed_size: u64::from_le_bytes(descriptor[8..16].try_into().unwrap()),
            })
        }
    }

    pub fn as_bytes(&self) -> [u8; ZIP64_DATA_DESCRIPTOR_LENGTH] {
        let mut array = [0; ZIP64_DATA_DESCRIPTOR_LENGTH];
        let mut cursor = 0;

        array_push!(array, cursor, self.crc.to_le_bytes());
        array_push!(array, cursor, self.compressed_size.to_le_bytes());
        array_push!(array, cursor, self.uncompressed_size.to_le_bytes());

        array
    }
}

impl Zip64EndOfCentralDirectoryLocator {
    /// Read 4 bytes from the reader and check whether its signature matches that of the EOCDL.
    /// If it does, return Some(EOCDL), otherwise return None.
    pub async fn try_from_reader<R: AsyncRead + Unpin>(
        reader: &mut R,
    ) -> Result<Option<Zip64EndOfCentralDirectoryLocator>> {
        let signature = {
            let mut buffer = [0; 4];
            reader.read_exact(&mut buffer).await?;
            u32::from_le_bytes(buffer)
        };
        if signature != ZIP64_EOCDL_SIGNATURE {
            return Ok(None);
        }
        let mut buffer: [u8; 16] = [0; 16];
        reader.read_exact(&mut buffer).await?;
        Ok(Some(Self::from(buffer)))
    }
}

/// Parse the extra fields.
pub fn parse_extra_fields(
    data: Vec<u8>,
    uncompressed_size: u32,
    compressed_size: u32,
    relative_header_offset: Option<u32>,
    disk_start_number: Option<u16>,
) -> Result<Vec<ExtraField>> {
    let mut cursor = 0;
    let mut extra_fields = Vec::<ExtraField>::new();

    while cursor + 4 <= data.len() {
        let header_id: HeaderId = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()).into();
        let field_size = u16::from_le_bytes(data[cursor + 2..cursor + 4].try_into().unwrap());
        if cursor + 4 + field_size as usize > data.len() {
            return Err(ZipError::InvalidExtraFieldHeader(field_size));
        }

        // Decode the extra field data.
        let data = &data[cursor + 4..cursor + 4 + field_size as usize];
        let extra_field = extra_field_from_bytes(
            header_id,
            field_size,
            data,
            uncompressed_size,
            compressed_size,
            relative_header_offset,
            disk_start_number,
        )?;

        // Verify that the extra field doesn't contain duplicates.
        for seen in &extra_fields {
            if extra_field.header_id() == seen.header_id() {
                return Err(ZipError::DuplicateExtraFieldHeader(header_id.into()));
            }
        }

        extra_fields.push(extra_field);
        cursor += 4 + field_size as usize;
    }
    Ok(extra_fields)
}

/// Replace elements of an array at a given cursor index for use with a zero-initialised array.
macro_rules! array_push {
    ($arr:ident, $cursor:ident, $value:expr) => {{
        for entry in $value {
            $arr[$cursor] = entry;
            $cursor += 1;
        }
    }};
}

use crate::spec::consts::{
    DATA_DESCRIPTOR_LENGTH, DATA_DESCRIPTOR_SIGNATURE, SIGNATURE_LENGTH, ZIP64_DATA_DESCRIPTOR_LENGTH,
    ZIP64_EOCDL_SIGNATURE,
};
use crate::spec::data_descriptor::{DataDescriptor, Zip64DataDescriptor};
use crate::spec::extra_field::extra_field_from_bytes;
pub(crate) use array_push;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_zip64_eocdr() {
        let eocdr: [u8; 56] = [
            0x50, 0x4B, 0x06, 0x06, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1E, 0x03, 0x2D, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x2F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let without_signature: [u8; 52] = eocdr[4..56].try_into().unwrap();
        let zip64eocdr = Zip64EndOfCentralDirectoryRecord::from(without_signature);
        assert_eq!(
            zip64eocdr,
            Zip64EndOfCentralDirectoryRecord {
                size_of_zip64_end_of_cd_record: 44,
                version_made_by: 798,
                version_needed_to_extract: 45,
                disk_number: 0,
                disk_number_start_of_cd: 0,
                num_entries_in_directory_on_disk: 1,
                num_entries_in_directory: 1,
                directory_size: 47,
                offset_of_start_of_directory: 64,
            }
        )
    }

    #[tokio::test]
    async fn test_parse_zip64_eocdl() {
        let eocdl: [u8; 20] = [
            0x50, 0x4B, 0x06, 0x07, 0x00, 0x00, 0x00, 0x00, 0x6F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];
        let mut cursor = futures_lite::io::Cursor::new(eocdl);
        let zip64eocdl = Zip64EndOfCentralDirectoryLocator::try_from_reader(&mut cursor).await.unwrap().unwrap();
        assert_eq!(
            zip64eocdl,
            Zip64EndOfCentralDirectoryLocator {
                number_of_disk_with_start_of_zip64_end_of_central_directory: 0,
                relative_offset: 111,
                total_number_of_disks: 1,
            }
        )
    }
}
