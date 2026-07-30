// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened synchronous directory stream envelope.

use super::DirectoryStreamSpi;

/// An already-open provider directory stream.
pub struct OpenedDirectoryStream {
    stream: Box<dyn DirectoryStreamSpi>,
}

impl OpenedDirectoryStream {
    /// Wraps a directory stream which the provider has fully opened.
    ///
    /// # Parameters
    /// - `stream`: Provider enumeration session.
    ///
    /// # Returns
    /// An opened-directory-stream envelope for the facade.
    #[inline]
    #[must_use]
    pub fn new(stream: Box<dyn DirectoryStreamSpi>) -> Self {
        Self { stream }
    }

    /// Returns the opened stream to the facade.
    ///
    /// # Returns
    /// The provider enumeration session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_stream(self) -> Box<dyn DirectoryStreamSpi> {
        self.stream
    }
}
