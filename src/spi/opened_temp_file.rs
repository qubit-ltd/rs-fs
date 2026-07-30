// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created synchronous temporary-file envelope.

use super::TempResourceSpi;
use crate::OpenedFileInfo;

/// An already-created provider temporary-file session.
pub struct OpenedTempFile {
    info: OpenedFileInfo,
    session: Box<dyn TempResourceSpi>,
}

impl OpenedTempFile {
    /// Wraps an owned temporary-file session.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the temporary file.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// A temporary-file envelope for facade validation.
    #[inline(always)]
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        session: Box<dyn TempResourceSpi>,
    ) -> Self {
        Self { info, session }
    }

    /// Returns the provider-owned parts to the facade.
    ///
    /// # Returns
    /// The claimed identity and provider lifecycle session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (OpenedFileInfo, Box<dyn TempResourceSpi>) {
        (self.info, self.session)
    }
}
