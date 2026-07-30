// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created asynchronous temporary-file envelope.

use super::AsyncTempResourceSpi;
use crate::OpenedFileInfo;

/// An already-created asynchronous temporary-file handle.
pub struct OpenedAsyncTempFile {
    info: OpenedFileInfo,
    session: Box<dyn AsyncTempResourceSpi>,
}

impl OpenedAsyncTempFile {
    /// Wraps an asynchronous temporary-file handle.
    #[inline(always)]
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        session: Box<dyn AsyncTempResourceSpi>,
    ) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    #[inline(always)]
    #[must_use]
    pub const fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the provider session into the facade handle.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (OpenedFileInfo, Box<dyn AsyncTempResourceSpi>) {
        (self.info, self.session)
    }
}
