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
use crate::metadata::OpenedFileInfo;

/// An already-open asynchronous writer bound to provider identity.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{AsyncFileWriteSession, OpenedAsyncWriter, SpiFuture};
/// use qubit_fs::write::{WriteAbortOutcome, WriteFailure};
/// use qubit_io::AsyncOutput;
/// use std::io::Result as IoResult;
/// use std::pin::Pin;
/// use std::task::{Context, Poll};
///
/// struct Session;
/// impl AsyncOutput for Session {
///     type Item = u8;
///     unsafe fn poll_write_unchecked(
///         self: Pin<&mut Self>,
///         _: &mut Context<'_>,
///         _: &[u8],
///         _: usize,
///         count: usize,
///     ) -> Poll<IoResult<usize>> {
///         Poll::Ready(Ok(count))
///     }
///     fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
///         Poll::Ready(Ok(()))
///     }
/// }
/// impl AsyncFileWriteSession for Session {
///     fn commit_async<'a>(
///         self: Pin<&'a mut Self>,
///     ) -> SpiFuture<'a, Result<qubit_fs::metadata::WriteOutcome, WriteFailure>> {
///         Box::pin(async { unreachable!() })
///     }
///     fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<WriteAbortOutcome>> {
///         Box::pin(async { Ok(WriteAbortOutcome::NotPublished) })
///     }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/draft")?)
///     .with_metadata(FileMetadata::new(FileKind::File));
/// let writer = OpenedAsyncWriter::new(info, Box::new(Session));
/// assert_eq!("/draft", writer.info().path().as_str());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
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

    /// Transfers the unvalidated identity and owned session to the facade.
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn AsyncFileWriteSession>) {
        (self.info, self.session)
    }
}
