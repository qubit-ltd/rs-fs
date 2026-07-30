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

use crate::{
    AsyncFileReader,
    OpenedFileInfo,
};

/// An already-open asynchronous reader bound to provider identity.
pub struct OpenedAsyncReader {
    info: OpenedFileInfo,
    reader: Box<dyn qubit_io::AsyncInput<Item = u8> + Send>,
}

impl OpenedAsyncReader {
    /// Wraps a provider-opened reader session and its claimed identity.
    #[inline(always)]
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        reader: Box<dyn qubit_io::AsyncInput<Item = u8> + Send>,
    ) -> Self {
        Self { info, reader }
    }

    /// Returns the immutable provider-opened identity.
    #[inline(always)]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated reader into the facade handle.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_reader(self) -> AsyncFileReader {
        AsyncFileReader::new(self.info, self.reader)
    }
}
