// Copyright (c) 2022 Harry [Majored] [hello@majored.pw]
// MIT License (https://github.com/Majored/rs-async-zip/blob/main/LICENSE)

#[cfg(feature = "chrono")]
use chrono::{TimeZone, Utc};

use crate::ZipDateTimeBuilder;

#[test]
fn default_date_is_valid_msdos_epoch() {
    let default = crate::ZipDateTime::default();

    assert_eq!(1980, default.year());
    assert_eq!(1, default.month());
    assert_eq!(1, default.day());
    assert_eq!(0, default.hour());
    assert_eq!(0, default.minute());
    assert_eq!(0, default.second());
}

#[test]
#[cfg(feature = "chrono")]
fn date_conversion_test_chrono() {
    let original_dt = Utc.timestamp_opt(1666544102, 0).unwrap();
    let zip_dt = crate::ZipDateTime::from_chrono(&original_dt);
    let result_dt = zip_dt.as_chrono().single().expect("expected single unique result");
    assert_eq!(result_dt, original_dt);
}

#[test]
fn date_conversion_test() {
    let year = 2000;
    let month = 9;
    let day = 8;
    let hour = 7;
    let minute = 5;
    let second = 4;

    let mut builder = ZipDateTimeBuilder::new();

    builder = builder.year(year);
    builder = builder.month(month);
    builder = builder.day(day);
    builder = builder.hour(hour);
    builder = builder.minute(minute);
    builder = builder.second(second);

    let built = builder.build();

    assert_eq!(year, built.year());
    assert_eq!(month, built.month());
    assert_eq!(day, built.day());
    assert_eq!(hour, built.hour());
    assert_eq!(minute, built.minute());
    assert_eq!(second, built.second());
}
