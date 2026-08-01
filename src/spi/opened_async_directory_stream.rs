// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened asynchronous directory stream envelope.

use super::AsyncDirectoryStreamSession;
use crate::{
    AsyncDirectoryStream,
    FileSystemLimits,
    ListOptions,
    Path,
    PathSemantics,
};

/// An already-open asynchronous directory stream.
pub struct OpenedAsyncDirectoryStream {
    /// Provider enumeration session awaiting facade validation.
    session: Box<dyn AsyncDirectoryStreamSession>,
}

impl OpenedAsyncDirectoryStream {
    /// Wraps an opened provider directory-enumeration session.
    ///
    /// # Parameters
    /// - `session`: Provider enumeration session.
    ///
    /// # Returns
    /// An opened asynchronous stream envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(session: Box<dyn AsyncDirectoryStreamSession>) -> Self {
        Self { session }
    }

    /// Transfers the stream into the facade handle.
    ///
    /// # Parameters
    /// - `root`: Validated directory root requested by the caller.
    /// - `options`: Validated listing behavior.
    /// - `provider`: Stable provider identifier used in generated errors.
    ///
    /// # Returns
    /// A facade-owned asynchronous directory stream.
    #[inline]
    #[must_use]
    pub(crate) fn into_stream(
        self,
        root: Path,
        options: ListOptions,
        provider: &str,
        path_semantics: PathSemantics,
        limits: FileSystemLimits,
    ) -> AsyncDirectoryStream {
        AsyncDirectoryStream::new(
            root,
            self.session,
            options,
            provider,
            path_semantics,
            limits,
        )
    }
}
