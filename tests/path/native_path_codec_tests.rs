// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    EscapedBytePathCodec,
    NativePathCodec,
    NativePathCodecError,
    Utf8PathCodec,
};

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
