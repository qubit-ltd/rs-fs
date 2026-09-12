// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Result of a bounded prefix read.

use crate::metadata::OpenedFileInfo;
use crate::read::PrefixReadTermination;
use crate::read::ReadOptions;

/// Bytes read from one opened resource together with bounded-read facts.
#[derive(Debug)]
pub struct PrefixReadOutcome {
    bytes: Vec<u8>,
    info: OpenedFileInfo,
    options: ReadOptions,
    max_bytes: usize,
    termination: PrefixReadTermination,
}

impl PrefixReadOutcome {
    /// Creates a prefix result inside the facade.
    #[inline]
    pub(crate) fn new(
        bytes: Vec<u8>,
        info: OpenedFileInfo,
        options: ReadOptions,
        max_bytes: usize,
        termination: PrefixReadTermination,
    ) -> Self {
        Self {
            bytes,
            info,
            options,
            max_bytes,
            termination,
        }
    }

    /// Returns the bytes without transferring ownership.
    #[inline]
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the information captured when the reader was opened.
    #[inline]
    #[must_use]
    pub const fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Returns the caller options retained for this read.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ReadOptions {
        &self.options
    }

    /// Returns the requested prefix limit.
    #[inline]
    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Returns the reason the read stopped.
    #[inline]
    #[must_use]
    pub const fn termination(&self) -> PrefixReadTermination {
        self.termination
    }

    /// Transfers the accumulated bytes without another allocation.
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
