// Copyright (c) 2021 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

pub struct DataDescriptor {
    pub crc: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}
