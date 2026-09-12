// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created asynchronous temporary-file envelope.

use super::AsyncTempResourceSpi;
use crate::metadata::OpenedFileInfo;

/// An already-created asynchronous temporary-file handle.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{AsyncTempResourceSpi, OpenedAsyncTempFile, PersistRequest, SpiFuture};
/// use std::pin::Pin;
///
/// struct Session;
/// impl AsyncTempResourceSpi for Session {
///     fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<()>> {
///         Box::pin(async { Ok(()) })
///     }
///     fn keep<'a>(
///         self: Pin<&'a mut Self>,
///     ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
///         Box::pin(async { unreachable!() })
///     }
///     fn persist<'a>(
///         self: Pin<&'a mut Self>,
///         _: PersistRequest<'a>,
///     ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
///         Box::pin(async { unreachable!() })
///     }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/scratch")?)
///     .with_metadata(FileMetadata::new(FileKind::File));
/// let _opened = OpenedAsyncTempFile::new(info, Box::new(Session));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct OpenedAsyncTempFile {
    /// Temporary-file identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider lifecycle session.
    session: Box<dyn AsyncTempResourceSpi>,
}

impl OpenedAsyncTempFile {
    /// Wraps an asynchronous temporary-file handle.
    ///
    /// # Parameters
    /// - `info`: Identity claimed for the temporary file.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// An asynchronous temporary-file envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncTempResourceSpi>) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    ///
    /// # Returns
    /// The identity claimed by the provider.
    #[inline(always)]
    #[must_use]
    pub const fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the provider session into the facade handle.
    ///
    /// # Returns
    /// The claimed identity and provider lifecycle session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn AsyncTempResourceSpi>) {
        (self.info, self.session)
    }
}
