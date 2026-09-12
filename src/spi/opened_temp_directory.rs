// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-created synchronous temporary-directory envelope.

use super::TempResourceSpi;
use crate::metadata::OpenedFileInfo;

/// An already-created provider temporary-directory session.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::FsResult;
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{OpenedTempDirectory, TempResourceSpi};
///
/// struct EmptyTemp;
/// impl TempResourceSpi for EmptyTemp {
///     fn persist(
///         &mut self,
///         _: qubit_fs::spi::PersistRequest<'_>,
///     ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
///         unreachable!()
///     }
///     fn keep(
///         &mut self,
///     ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
///         unreachable!()
///     }
///     fn cleanup(&mut self) -> FsResult<()> {
///         Ok(())
///     }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/tmp")?)
///     .with_metadata(FileMetadata::new(FileKind::Directory));
/// let _opened = OpenedTempDirectory::new(info, Box::new(EmptyTemp));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct OpenedTempDirectory {
    /// Temporary-directory identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider lifecycle session.
    session: Box<dyn TempResourceSpi>,
}

impl OpenedTempDirectory {
    /// Wraps an owned temporary-directory session.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the temporary directory.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// A temporary-directory envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn TempResourceSpi>) -> Self {
        Self { info, session }
    }

    /// Returns the provider-owned parts to the facade.
    ///
    /// # Returns
    /// The claimed identity and provider lifecycle session.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn TempResourceSpi>) {
        (self.info, self.session)
    }
}
