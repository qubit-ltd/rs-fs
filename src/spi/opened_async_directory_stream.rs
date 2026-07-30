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
    ListOptions,
    Path,
};

/// An already-open asynchronous directory stream.
pub struct OpenedAsyncDirectoryStream {
    session: Box<dyn AsyncDirectoryStreamSession>,
}

impl OpenedAsyncDirectoryStream {
    /// Wraps an opened provider directory-enumeration session.
    #[inline(always)]
    #[must_use]
    pub fn new(session: Box<dyn AsyncDirectoryStreamSession>) -> Self {
        Self { session }
    }

    /// Transfers the stream into the facade handle.
    #[inline]
    #[must_use]
    pub(crate) fn into_stream(
        self,
        root: Path,
        options: ListOptions,
        provider: &str,
    ) -> AsyncDirectoryStream {
        AsyncDirectoryStream::new(root, self.session, options, provider)
    }
}
