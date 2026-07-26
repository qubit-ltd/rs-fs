// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::borrow::Cow;

use qubit_fs::{NativePathCodec, NativePathCodecError, Utf8PathCodec};

#[test]
fn utf8_path_codec_borrows_plain_utf8() {
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

#[test]
fn utf8_path_codec_rejects_escaped_non_utf8_bytes() {
    assert!(matches!(
        Utf8PathCodec.encode("%FF"),
        Err(NativePathCodecError::InvalidUtf8 { offset: 0 })
    ));
}

#[test]
fn utf8_path_codec_owns_canonical_text_when_native_bytes_need_escaping() {
    assert!(matches!(
        Utf8PathCodec.decode(b"100%"),
        Ok(Cow::Owned(text)) if text == "100%25"
    ));
}
