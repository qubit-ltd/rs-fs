// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created asynchronous temporary-directory envelope.

use super::AsyncTempResourceSpi;
use crate::OpenedFileInfo;

/// An already-created asynchronous temporary-directory handle.
pub struct OpenedAsyncTempDirectory {
    info: OpenedFileInfo,
    session: Box<dyn AsyncTempResourceSpi>,
}

impl OpenedAsyncTempDirectory {
    /// Wraps an asynchronous temporary-directory handle.
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
