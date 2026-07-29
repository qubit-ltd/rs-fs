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
    ConnectionUri,
    Path,
    RelativePath,
    Uri,
};

/// Bounds parser and codec allocations for direct target invocation.
const MAX_FUZZ_INPUT_LEN: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FUZZ_INPUT_LEN)];

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = Path::parse(text);
        let _ = RelativePath::parse(text);
        let _ = Uri::parse(text);
        let _ = ConnectionUri::parse(text);
    }
});
