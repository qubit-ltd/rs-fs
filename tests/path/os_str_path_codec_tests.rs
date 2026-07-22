// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    NativePathCodec,
    OsStrPathCodec,
};

use std::borrow::Cow;
use std::ffi::OsStr;

#[cfg(windows)]
use qubit_fs::NativePathCodecError;

#[cfg(unix)]
#[test]
fn os_str_path_codec_round_trips_non_utf8_unix_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let native = OsString::from_vec(vec![b'f', b'o', 0x80, b'o']);
    let codec = OsStrPathCodec;
    let text = codec
        .decode(native.as_os_str())
        .expect("native path should decode");

    assert_eq!("fo%80o", text);
    assert_eq!(
        native,
        codec
            .encode(&text)
            .expect("canonical text should encode")
            .into_owned(),
    );
}

#[test]
fn os_str_path_codec_borrows_plain_native_text() {
    let codec = OsStrPathCodec;

    assert!(matches!(
        codec.encode("report-中文.txt"),
        Ok(Cow::Borrowed(value)) if value == OsStr::new("report-中文.txt")
    ));
    assert!(matches!(
        codec.decode(OsStr::new("report-中文.txt")),
        Ok(Cow::Borrowed("report-中文.txt"))
    ));
}

#[cfg(windows)]
#[test]
fn os_str_path_codec_round_trips_unpaired_windows_surrogates() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let codec = OsStrPathCodec;
    for (wide, text) in
        [(vec![0xd800], "%ED%A0%80"), (vec![0xdc00], "%ED%B0%80")]
    {
        let native = OsString::from_wide(&wide);
        let decoded = codec
            .decode(native.as_os_str())
            .expect("unpaired surrogate should decode losslessly");

        assert_eq!(text, decoded);
        assert_eq!(
            native,
            codec
                .encode(&decoded)
                .expect("canonical surrogate text should encode")
                .into_owned(),
        );
    }
    assert!(matches!(
        codec.encode("%F0%80%80%80"),
        Err(NativePathCodecError::InvalidWtf8 { offset: 0 })
    ));
}
