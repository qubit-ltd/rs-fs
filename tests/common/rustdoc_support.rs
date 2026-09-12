// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
// Deterministic in-memory provider used only by executable Rustdoc examples.

pub use qubit_fs::rustdoc_provider;

#[cfg(feature = "async")]
#[path = "async_recording_spi.rs"]
pub(crate) mod async_recording_spi;
#[path = "poll_support.rs"]
pub(crate) mod poll_support;
