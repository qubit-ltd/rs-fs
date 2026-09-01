// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Asynchronous aggregate write operation implementation.

use qubit_io::AsyncOutput;

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsOperation;
use crate::metadata::WriteOutcome;
use crate::path::Path;
use crate::write::AsyncWriteAllFailure;
use crate::write::WriteOptions;

/// Executes aggregate asynchronous write operations for one facade.
pub(crate) struct AsyncWriteOperation<'a> {
    /// Facade that opens and contextualizes the writer.
    filesystem: &'a AsyncFileSystem,
}

impl<'a> AsyncWriteOperation<'a> {
    /// Creates an asynchronous write operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a AsyncFileSystem) -> Self {
        Self { filesystem }
    }

    /// Writes all bytes asynchronously and retains the writer on failure.
    pub(crate) async fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<WriteOutcome, AsyncWriteAllFailure> {
        if let Err(error) = self
            .filesystem
            .properties()
            .limits()
            .validate_write_size(path, bytes.len())
        {
            return Err(AsyncWriteAllFailure::new(
                self.filesystem.core().enrich(error, Some(path), FsOperation::Write),
                None,
            ));
        }
        let mut writer = self
            .filesystem
            .open_writer(path, options)
            .await
            .map_err(|error| AsyncWriteAllFailure::new(error, None))?;
        if let Err(error) = writer.write_fully_async(bytes).await {
            return Err(AsyncWriteAllFailure::new(
                self.filesystem.core().enrich(
                    FsError::from_stream_io(error, FsOperation::Write, path),
                    Some(path),
                    FsOperation::Write,
                ),
                Some(writer),
            ));
        }
        if let Err(error) = writer.flush_async().await {
            return Err(AsyncWriteAllFailure::new(
                self.filesystem.core().enrich(
                    FsError::from_stream_io(error, FsOperation::Write, path),
                    Some(path),
                    FsOperation::Write,
                ),
                Some(writer),
            ));
        }
        writer
            .commit_async()
            .await
            .map_err(|failure| AsyncWriteAllFailure::new(failure.into_error(), Some(writer)))
    }
}
