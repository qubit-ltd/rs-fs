// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened synchronous writer envelope.

use super::FileWriterSpi;
use crate::OpenedFileInfo;

/// An already-open provider writer.
pub struct OpenedWriter {
    info: OpenedFileInfo,
    writer: Box<dyn FileWriterSpi>,
}

impl OpenedWriter {
    /// Wraps a writer which the provider has fully opened.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the opened resource.
    /// - `writer`: Provider writer session.
    ///
    /// # Returns
    /// An opened-writer envelope for facade validation.
    #[inline(always)]
    #[must_use]
    pub fn new(info: OpenedFileInfo, writer: Box<dyn FileWriterSpi>) -> Self {
        Self { info, writer }
    }

    /// Returns the opened writer to the facade.
    ///
    /// # Returns
    /// The claimed resource identity and provider writer session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn FileWriterSpi>) {
        (self.info, self.writer)
    }
}
