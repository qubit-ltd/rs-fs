// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened asynchronous writer envelope.

use super::AsyncFileWriteSession;
use crate::{
    AsyncFileWriter,
    AtomicityRequirement,
    OpenedFileInfo,
};

/// An already-open asynchronous writer bound to provider identity.
pub struct OpenedAsyncWriter {
    info: OpenedFileInfo,
    session: Box<dyn AsyncFileWriteSession>,
}

impl OpenedAsyncWriter {
    /// Wraps an opened provider writer session and its validated identity.
    #[inline(always)]
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        session: Box<dyn AsyncFileWriteSession>,
    ) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    #[inline(always)]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated writer into the facade handle.
    #[inline]
    #[must_use]
    pub(crate) fn into_writer(
        self,
        atomicity: AtomicityRequirement,
        provider: &str,
        max_write_bytes: Option<u64>,
    ) -> AsyncFileWriter {
        AsyncFileWriter::new(
            self.info,
            self.session,
            atomicity,
            provider,
            max_write_bytes,
        )
    }
}
