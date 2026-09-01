// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened asynchronous reader envelope.

use qubit_io::AsyncInput;

use crate::metadata::OpenedFileInfo;
use crate::read::AsyncFileReader;

/// An already-open asynchronous reader bound to provider identity.
pub struct OpenedAsyncReader {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider asynchronous byte-input session.
    reader: Box<dyn AsyncInput<Item = u8> + Send>,
}

impl OpenedAsyncReader {
    /// Wraps a provider-opened reader session and its claimed identity.
    ///
    /// # Parameters
    /// - `info`: Identity claimed for the opened resource.
    /// - `reader`: Provider asynchronous byte-input session.
    ///
    /// # Returns
    /// An opened-reader envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, reader: Box<dyn AsyncInput<Item = u8> + Send>) -> Self {
        Self { info, reader }
    }

    /// Returns the immutable provider-opened identity.
    ///
    /// # Returns
    /// The identity claimed by the provider.
    #[inline(always)]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated reader into the facade handle.
    ///
    /// # Returns
    /// A facade-owned asynchronous reader.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_reader(self) -> AsyncFileReader {
        AsyncFileReader::new(self.info, self.reader)
    }
}
