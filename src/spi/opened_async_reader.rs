// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Provider-opened asynchronous reader envelope.

use qubit_io::AsyncByteInput;

use crate::metadata::OpenedFileInfo;
use crate::read::AsyncFileReader;

/// An already-open asynchronous reader bound to provider identity.
///
/// # Examples
///
/// ```rust
/// use std::io::Result as IoResult;
/// use std::pin::Pin;
/// use std::task::Context;
/// use std::task::Poll;
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::OpenedAsyncReader;
/// use qubit_io::AsyncInput;
///
/// struct EmptyInput;
///
/// impl AsyncInput for EmptyInput {
///     type Item = u8;
///
///     unsafe fn poll_read_unchecked(
///         self: Pin<&mut Self>,
///         _: &mut Context<'_>,
///         _: &mut [u8],
///         _: usize,
///         _: usize,
///     ) -> Poll<IoResult<usize>> {
///         Poll::Ready(Ok(0))
///     }
/// }
///
/// let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/x")?)
///     .with_metadata(FileMetadata::new(FileKind::File));
/// let opened = OpenedAsyncReader::new(info, Box::new(EmptyInput));
/// assert_eq!("/x", opened.info().path().as_str());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct OpenedAsyncReader {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider asynchronous byte-input session.
    reader: Box<dyn AsyncByteInput + Send>,
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
    pub fn new(info: OpenedFileInfo, reader: Box<dyn AsyncByteInput + Send>) -> Self {
        Self { info, reader }
    }

    /// Returns the immutable provider-opened identity.
    ///
    /// # Returns
    /// The identity claimed by the provider.
    #[inline]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated reader into the facade handle.
    ///
    /// # Returns
    /// A facade-owned asynchronous reader.
    #[inline]
    #[must_use]
    pub(crate) fn into_reader(self) -> AsyncFileReader {
        AsyncFileReader::new(self.info, self.reader)
    }
}
