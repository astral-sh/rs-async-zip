// Copyright (c) 2021-2024 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

// https://github.com/Majored/rs-async-zip/blob/main/SPECIFICATION.md#446
// https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-dosdatetimetovarianttime

/// A date and time stored as per the MS-DOS representation used by ZIP files.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ZipDateTime {
    pub(crate) date: u16,
    pub(crate) time: u16,
}

impl ZipDateTime {
    /// Returns the year of this date & time.
    pub fn year(&self) -> i32 {
        (((self.date & 0xFE00) >> 9) + 1980).into()
    }

    /// Returns the month of this date & time.
    pub fn month(&self) -> u32 {
        ((self.date & 0x1E0) >> 5).into()
    }

    /// Returns the day of this date & time.
    pub fn day(&self) -> u32 {
        (self.date & 0x1F).into()
    }

    /// Returns the hour of this date & time.
    pub fn hour(&self) -> u32 {
        ((self.time & 0xF800) >> 11).into()
    }

    /// Returns the minute of this date & time.
    pub fn minute(&self) -> u32 {
        ((self.time & 0x7E0) >> 5).into()
    }

    /// Returns the second of this date & time.
    ///
    /// Note that MS-DOS has a maximum granularity of two seconds.
    pub fn second(&self) -> u32 {
        ((self.time & 0x1F) << 1).into()
    }
}
