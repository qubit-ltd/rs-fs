// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::NativePathCodecError;

#[test]
fn native_path_codec_errors_report_safe_offsets() {
    let cases = [
        (
            NativePathCodecError::InvalidUtf8 { offset: 1 },
            "invalid UTF-8 at native byte offset 1",
        ),
        (
            NativePathCodecError::InvalidEscape { offset: 2 },
            "invalid percent escape at UTF-8 byte offset 2",
        ),
        (
            NativePathCodecError::NonCanonicalText { offset: 3 },
            "non-canonical native path text at UTF-8 byte offset 3",
        ),
        (
            NativePathCodecError::InvalidWtf8 { offset: 4 },
            "invalid WTF-8 at WTF-8 byte offset 4",
        ),
        (
            NativePathCodecError::UnsupportedNativeEncoding,
            "native path encoding is not losslessly supported",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(expected, error.to_string());
        assert!(!error.to_string().contains("secret"));
    }
}
