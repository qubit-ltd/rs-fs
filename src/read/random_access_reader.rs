// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Application-facing handle for bounded offset reads.
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemLimit;
use crate::metadata::OpenedFileInfo;
use crate::read::RandomAccessReadPolicy;
use crate::spi::RandomAccessReadSession;

/// A random-access session bound to one opened object and its open-time length.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::{FileSystem, FsResult, Path};
/// use qubit_fs::read::{RandomAccessReader, ReadOptions};
///
/// fn sample(filesystem: &FileSystem, path: &Path) -> FsResult<Vec<u8>> {
///     let mut reader: RandomAccessReader = filesystem.open_random_access_reader(path, ReadOptions::default())?;
///     let mut bytes = [0; 16];
///     let mut filled = 0;
///     while filled < bytes.len() {
///         match reader.read_at(filled as u64, &mut bytes[filled..])? {
///             0 => break, // EOF; a short read is not an error.
///             count => filled += count,
///         }
///     }
///     Ok(bytes[..filled].to_vec())
/// }
/// ```
///
/// If `read_at` returns `Err`, the output may already contain partial bytes;
/// discard the incomplete sample. The open-time length is not a content
/// snapshot.
pub struct RandomAccessReader {
    info: OpenedFileInfo,
    len: u64,
    limit: FileSystemLimit,
    provider_id: String,
    session: Box<dyn RandomAccessReadSession>,
}
impl RandomAccessReader {
    pub(crate) fn new(
        info: OpenedFileInfo,
        len: u64,
        limit: FileSystemLimit,
        provider_id: String,
        session: Box<dyn RandomAccessReadSession>,
    ) -> Self {
        Self {
            info,
            len,
            limit,
            provider_id,
            session,
        }
    }
    /// Returns open-time identity and metadata.
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    /// Returns the accessible upper bound observed when opened.
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Returns whether the open-time length is zero.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Reads into caller storage at an explicit offset. Requests past `len` are
    /// clipped.
    ///
    /// # Parameters
    /// - `offset`: Byte offset from the beginning of the opened object.
    /// - `output`: Caller-owned destination buffer.
    ///
    /// # Returns
    /// The number of bytes initialized, possibly less than `output.len()`;
    /// zero means this request observed EOF.
    ///
    /// # Errors
    /// Returns `InvalidOptions` for an overflowing offset range,
    /// `ResourceLimitExceeded` when the clipped request exceeds the provider
    /// limit, a provider read error with path/provider/source context, or
    /// `ProviderContractViolation` if the provider over-reports bytes. On an
    /// error, the buffer may contain partial bytes and should be discarded.
    pub fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let count = RandomAccessReadPolicy::validate_read(offset, output.len(), self.len, self.limit)
            .map_err(|e| e.with_path(self.info.path().clone()).with_provider(&self.provider_id))?;
        if count == 0 {
            return Ok(0);
        }
        let n = self.session.read_at(offset, &mut output[..count]).map_err(|e| {
            e.with_path(self.info.path().clone())
                .with_provider(&self.provider_id)
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
impl Debug for RandomAccessReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RandomAccessReader")
            .field("info", &self.info)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}
