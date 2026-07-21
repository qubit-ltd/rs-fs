// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    EscapedBytePathCodec, NativePathCodec, NativePathCodecError, OsStrPathCodec, Utf8PathCodec,
};
use std::borrow::Cow;

#[test]
fn test_escaped_byte_path_codec_rejects_text_aliases() {
    let codec = EscapedBytePathCodec;

    assert!(matches!(
        codec.encode("%2f"),
        Err(NativePathCodecError::NonCanonicalText { .. })
    ));
    assert!(matches!(
        codec.encode("%41"),
        Err(NativePathCodecError::NonCanonicalText { .. })
    ));
    assert!(matches!(
        codec.encode("raw%"),
        Err(NativePathCodecError::InvalidEscape { .. })
    ));
}

#[test]
fn test_byte_codecs_preserve_their_supported_domains() {
    let bytes = [b'f', b'o', 0x80, b'o'];
    let escaped = EscapedBytePathCodec;

    assert_eq!(
        "fo%80o",
        escaped.decode(&bytes).expect("opaque bytes should decode")
    );
    assert_eq!(
        &bytes,
        escaped
            .encode("fo%80o")
            .expect("canonical text should encode")
            .as_ref()
    );
    assert!(matches!(
        Utf8PathCodec.decode(&bytes),
        Err(NativePathCodecError::InvalidUtf8 { .. })
    ));
}

#[test]
fn test_utf8_path_codec_borrows_plain_utf8() {
    let bytes = "report-中文.txt".as_bytes();

    assert!(matches!(
        Utf8PathCodec.decode(bytes),
        Ok(Cow::Borrowed("report-中文.txt"))
    ));
    assert!(matches!(
        Utf8PathCodec.encode("report-中文.txt"),
        Ok(Cow::Borrowed(_))
    ));
}

#[cfg(unix)]
#[test]
fn test_os_str_path_codec_round_trips_non_utf8_unix_bytes() {
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
            .into_owned()
    );
}

#[cfg(windows)]
#[test]
fn test_os_str_path_codec_round_trips_unpaired_windows_surrogates() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let codec = OsStrPathCodec;
    for (wide, text) in [(vec![0xd800], "%ED%A0%80"), (vec![0xdc00], "%ED%B0%80")] {
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
                .into_owned()
        );
    }
    assert!(matches!(
        codec.encode("%F0%80%80%80"),
        Err(NativePathCodecError::InvalidWtf8 { offset: 0 })
    ));
}

#[test]
fn test_escaped_byte_path_codec_round_trips_every_single_byte() {
    let codec = EscapedBytePathCodec;

    for byte in 0_u8..=u8::MAX {
        let native = [byte];
        let text = codec.decode(&native).expect("every byte should decode");
        assert_eq!(
            native,
            codec
                .encode(&text)
                .expect("canonical text should encode")
                .as_ref(),
            "byte {byte:#04x}"
        );
    }
}

#[test]
fn test_codecs_agree_for_valid_utf8_text() {
    let text = "空 格-e\u{301}-%25";
    let native = Utf8PathCodec
        .encode(text)
        .expect("canonical UTF-8 text should encode");

    assert_eq!(
        text,
        EscapedBytePathCodec
            .decode(native.as_ref())
            .expect("strict UTF-8 bytes should decode as opaque bytes")
    );
}

#[test]
fn test_utf8_path_codec_rejects_escaped_non_utf8_bytes() {
    assert!(matches!(
        Utf8PathCodec.encode("%FF"),
        Err(NativePathCodecError::InvalidUtf8 { offset: 0 })
    ));
}
