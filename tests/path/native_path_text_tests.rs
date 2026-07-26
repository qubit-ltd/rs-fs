// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{EscapedBytePathCodec, NativePathCodec, NativePathCodecError};

#[test]
fn canonical_native_path_text_rejects_aliases_and_malformed_escapes() {
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
fn canonical_native_path_text_escapes_percent_and_control_bytes() {
    let codec = EscapedBytePathCodec;
    let native = b"%\n";

    let text = codec.decode(native).unwrap();
    assert_eq!("%25%0A", text);
    assert_eq!(native, codec.encode(&text).unwrap().as_ref());
}
