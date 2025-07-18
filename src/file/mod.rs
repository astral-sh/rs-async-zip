// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

use crate::entry::StoredZipEntry;

/// An immutable store of data about a ZIP file.
#[derive(Clone)]
pub struct ZipFile {
    pub(crate) entries: Vec<StoredZipEntry>,
    pub(crate) zip64: bool,
}

impl ZipFile {
    /// Returns a list of this ZIP file's entries.
    pub fn entries(&self) -> &[StoredZipEntry] {
        &self.entries
    }

    /// Returns whether or not this ZIP file is zip64
    pub fn zip64(&self) -> bool {
        self.zip64
    }
}
