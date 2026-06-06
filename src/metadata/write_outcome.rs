// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation outcome.

use qubit_metadata::Metadata;

/// Outcome returned when a writer is committed.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOutcome {
    /// Number of bytes written when known.
    pub bytes_written: Option<u64>,
    /// Provider version or HTTP-style ETag when known.
    pub etag: Option<String>,
    /// Provider-native diagnostics.
    pub diagnostics: Metadata,
}

impl WriteOutcome {
    /// Creates an empty write outcome.
    ///
    /// # Returns
    /// Write outcome with unknown byte count and no diagnostics.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for WriteOutcome {
    #[inline]
    fn default() -> Self {
        Self {
            bytes_written: None,
            etag: None,
            diagnostics: Metadata::new(),
        }
    }
}
