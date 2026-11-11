// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Application-facing asynchronous handle for bounded offset reads.
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::pin::Pin;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemLimit;
use crate::metadata::OpenedFileInfo;
use crate::read::RandomAccessReadPolicy;
use crate::spi::AsyncRandomAccessReadSession;

/// An async random-access session bound to one opened object.
///
/// # Examples
///
/// This small example assumes each requested window fits the configured
/// `max_read_range_bytes`; a larger single `read_at` request returns
/// `ResourceLimitExceeded`. See the user guide for a fixture-backed helper
/// that splits a larger sample across bounded reads.
///
/// ```rust
/// use qubit_fs::{AsyncFileSystem, Path};
/// use qubit_fs::read::ReadOptions;
///
/// async fn sample(filesystem: &AsyncFileSystem, path: &Path) -> Result<Vec<u8>, qubit_fs::FsError> {
///     let mut reader = filesystem.open_random_access_reader(path, ReadOptions::default()).await?;
///     let mut bytes = [0; 16];
///     let mut filled = 0;
///     while filled < bytes.len() {
///         match reader.read_at(filled as u64, &mut bytes[filled..]).await? {
///             0 => break,
///             count => filled += count,
///         }
///     }
///     Ok(bytes[..filled].to_vec())
/// }
/// ```
///
/// A short read is allowed. If a call returns `Err` or its future is dropped,
/// the output may contain partial bytes; discard the incomplete sample. The
/// cached length is not a content snapshot.
pub struct AsyncRandomAccessReader {
    /// Identity and metadata captured when the object was opened.
    info: OpenedFileInfo,
    /// Open-time readable upper bound; this is not a content snapshot.
    len: u64,
    /// Maximum size of one request after clipping to `len`.
    limit: FileSystemLimit,
    /// Provider identifier added to read errors for context.
    provider_id: String,
    /// Pinned session retaining ownership of the already opened object.
    session: Pin<Box<dyn AsyncRandomAccessReadSession>>,
}
impl AsyncRandomAccessReader {
    /// Builds a reader from the validated result of a provider open and pins
    /// its session for the lifetime of the reader.
    ///
    /// # Parameters
    /// - `info`: File identity and metadata captured at open time.
    /// - `len`: Readable upper bound captured at open time.
    /// - `limit`: Maximum size of each clipped provider request.
    /// - `provider_id`: Provider identity included in read errors.
    /// - `session`: Provider session owning the already-open object; it is
    ///   pinned for the lifetime of the reader.
    ///
    /// # Returns
    /// A reader that retains the open object and applies the supplied bound
    /// and request limit.
    pub(crate) fn new(
        info: OpenedFileInfo,
        len: u64,
        limit: FileSystemLimit,
        provider_id: String,
        session: Box<dyn AsyncRandomAccessReadSession>,
    ) -> Self {
        Self {
            info,
            len,
            limit,
            provider_id,
            session: Box::into_pin(session),
        }
    }
    /// Returns the identity and metadata captured at open time.
    #[inline]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    /// Returns the open-time readable upper bound.
    ///
    /// This bound does not change if the object grows and does not make the
    /// object's contents a snapshot.
    #[inline]
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Returns whether the open-time length is zero.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Reads into caller storage at an explicit offset; cancellation does not
    /// change session identity.
    ///
    /// # Parameters
    /// - `offset`: Byte offset from the beginning of the opened object.
    /// - `output`: Caller-owned destination buffer borrowed until the future
    ///   completes or is dropped.
    ///
    /// # Returns
    /// Returns the number of bytes read. An empty output buffer returns zero
    /// without observing EOF. A non-overflowing request at or beyond the
    /// open-time length also returns zero without provider I/O. For a non-empty
    /// request within that bound, a provider result of zero indicates EOF
    /// observed by that request. A positive count may be shorter than the
    /// clipped request.
    ///
    /// # Errors
    /// Returns `InvalidOptions` for an overflowing offset range,
    /// `ResourceLimitExceeded` when the clipped request exceeds the provider
    /// limit, a provider error with path/provider/source context, or
    /// `ProviderContractViolation` when the provider over-reports bytes. The
    /// buffer may contain partial bytes after an error or cancellation and
    /// must be discarded. Provider-supplied path and provider context is
    /// preserved; missing fields are filled from this session. The operation
    /// is reported as `Read`.
    pub async fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let count = RandomAccessReadPolicy::validate_read(offset, output.len(), self.len, self.limit)
            .map_err(|e| e.with_path(self.info.path().clone()).with_provider(&self.provider_id))?;
        if count == 0 {
            return Ok(0);
        }
        let n = self
            .session
            .as_mut()
            .read_at_async(offset, &mut output[..count])
            .await
            .map_err(|e| {
                e.with_missing_context(self.info.path(), None, &self.provider_id)
                    .with_operation(FsOperation::Read)
            })?;
        if n > count {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::Read,
                "provider returned more bytes than supplied buffer",
            )
            .with_path(self.info.path().clone())
            .with_provider(&self.provider_id));
        }
        Ok(n)
    }
}
impl Debug for AsyncRandomAccessReader {
    /// Formats the open-time information and length without exposing the
    /// provider session.
    ///
    /// # Parameters
    /// - `f`: Destination for the debug representation.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AsyncRandomAccessReader")
            .field("info", &self.info)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}
