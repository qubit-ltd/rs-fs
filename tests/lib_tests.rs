// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

mod common;
#[cfg(feature = "async")]
pub(crate) use common::async_recording_spi;
pub(crate) use common::poll_support;
mod copy;
mod directory;
mod error;
mod handle_support;
mod internal;
mod metadata;
mod options;
mod path;
mod reader;
mod rename;
mod spi;
mod temp;
mod uri;
mod write;
mod writer;

#[test]
fn test_poll_support_completes_ready_future() {
    assert_eq!(42, poll_support::ready(async { 42 }));
}
