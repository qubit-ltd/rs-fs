// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_fs::{
    EscapedBytePathCodec,
    FsAuthority,
    FsPath,
    FsUri,
    FsUriPath,
    NativePathCodec,
    RelativeFsPath,
};

/// Bounds parser and codec allocations for direct target invocation.
const MAX_FUZZ_INPUT_LEN: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FUZZ_INPUT_LEN)];
    fuzz_escaped_byte_round_trip(data);

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = FsAuthority::new(text);
        let _ = FsPath::parse(text);
        let _ = RelativeFsPath::parse(text);
        let _ = FsUri::parse(text);
        fuzz_uri_path_round_trip(text);
    }
});

/// Checks that arbitrary native bytes retain their exact identity.
fn fuzz_escaped_byte_round_trip(data: &[u8]) {
    let codec = EscapedBytePathCodec;
    let text = codec
        .decode(data)
        .expect("escaping arbitrary native bytes must succeed");
    let decoded = codec
        .encode(text.as_ref())
        .expect("codec output must be valid canonical text");

    assert_eq!(data, decoded.as_ref());
}

/// Checks that validated URI paths preserve their canonical encoded spelling.
fn fuzz_uri_path_round_trip(text: &str) {
    let Ok(path) = FsUriPath::parse(text) else {
        return;
    };
    let reparsed = FsUriPath::parse(path.as_encoded())
        .expect("validated URI path must parse again");

    assert_eq!(path, reparsed);
}
