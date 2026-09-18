// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// Deterministic in-memory provider used only by executable Rustdoc examples.

/// Isolated in-memory provider shared by executable Rustdoc examples.
pub mod rustdoc_provider {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/common/rustdoc_provider_impl.rs"
    ));
}

#[cfg(feature = "async")]
#[path = "async_recording_spi.rs"]
pub(crate) mod async_recording_spi;
#[path = "poll_support.rs"]
pub(crate) mod poll_support;
