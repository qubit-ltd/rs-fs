// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created synchronous temporary-directory envelope.

use super::TempResourceSpi;
use crate::OpenedFileInfo;

/// An already-created provider temporary-directory session.
pub struct OpenedTempDirectory {
    /// Temporary-directory identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider lifecycle session.
    session: Box<dyn TempResourceSpi>,
}

impl OpenedTempDirectory {
    /// Wraps an owned temporary-directory session.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the temporary directory.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// A temporary-directory envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn TempResourceSpi>) -> Self {
        Self { info, session }
    }

    /// Returns the provider-owned parts to the facade.
    ///
    /// # Returns
    /// The claimed identity and provider lifecycle session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn TempResourceSpi>) {
        (self.info, self.session)
    }
}
