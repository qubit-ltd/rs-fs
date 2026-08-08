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
use crate::AsyncFileWriter;
use crate::AtomicityRequirement;
use crate::OpenedFileInfo;

/// An already-open asynchronous writer bound to provider identity.
pub struct OpenedAsyncWriter {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider asynchronous write session.
    session: Box<dyn AsyncFileWriteSession>,
}

impl OpenedAsyncWriter {
    /// Wraps an opened provider writer session and its validated identity.
    ///
    /// # Parameters
    /// - `info`: Identity claimed for the opened resource.
    /// - `session`: Provider asynchronous writer session.
    ///
    /// # Returns
    /// An opened-writer envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncFileWriteSession>) -> Self {
        Self { info, session }
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

    /// Transfers the validated writer into the facade handle.
    ///
    /// # Parameters
    /// - `atomicity`: Publication atomicity requested by the caller.
    /// - `provider`: Stable provider identifier used in generated errors.
    /// - `max_write_bytes`: Optional provider write-size limit.
    ///
    /// # Returns
    /// A facade-owned asynchronous writer.
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
