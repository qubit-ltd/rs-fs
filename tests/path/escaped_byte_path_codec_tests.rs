// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{EscapedBytePathCodec, NativePathCodec};

#[test]
fn escaped_byte_path_codec_round_trips_every_single_byte() {
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
            "byte {byte:#04x}",
        );
    }
}
