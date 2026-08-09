// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened synchronous reader envelope.

use qubit_io::Input;

use crate::OpenedFileInfo;

/// An already-open provider reader.
pub struct OpenedReader {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider byte-input session.
    reader: Box<dyn Input<Item = u8> + Send>,
}

impl OpenedReader {
    /// Wraps a reader which the provider has fully opened.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the opened resource.
    /// - `reader`: Provider reader session.
    ///
    /// # Returns
    /// An opened-reader envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, reader: Box<dyn Input<Item = u8> + Send>) -> Self {
        Self { info, reader }
    }

    /// Returns the opened reader to the facade.
    ///
    /// # Returns
    /// The claimed resource identity and provider reader session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn Input<Item = u8> + Send>) {
        (self.info, self.reader)
    }
}
