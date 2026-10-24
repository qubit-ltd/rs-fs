// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous read operation implementation.

use qubit_io::AsyncInput;

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::facade::internal::FileSystemResource;
use crate::path::Path;
use crate::read::PrefixReadOutcome;
use crate::read::PrefixReadTermination;
use crate::read::ReadOptions;
use crate::read::internal::ReadBuffer;
use crate::read::prefix_read_plan::PrefixReadPlan;

/// Coordinates asynchronous reads through one filesystem facade.
///
/// The operation object borrows the facade so every reader uses the same
/// provider configuration and error-context policy.
pub(crate) struct AsyncReadOperation<'a> {
    /// Opens readers and enriches their errors with filesystem context.
    filesystem: &'a AsyncFileSystem,
}

impl<'a> AsyncReadOperation<'a> {
    /// Creates read operations using `filesystem` for reader creation and error
    /// context.
    #[inline]
    #[must_use]
    pub(crate) const fn new(filesystem: &'a AsyncFileSystem) -> Self {
        Self { filesystem }
    }

    /// Reads a complete file asynchronously, rejecting content beyond
    /// `max_bytes`.
    ///
    /// The reader is consumed until end of stream. If the stream contains more
    /// than the allowed number of bytes, the returned error identifies a read
    /// budget violation; stream and buffer errors retain filesystem context.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or read, if its content
    /// exceeds `max_bytes`, or if the bounded read buffer cannot append data.
    pub(crate) async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        let mut reader = self.filesystem.open_reader(path, options).await?;
        let mut bytes = ReadBuffer::new(max_bytes);
        let maximum = FacadeCore::quantity_from_usize(
            max_bytes,
            FsOperation::Read,
            path,
            self.filesystem.properties().info().provider_id(),
        )?;
        let mut read_budget = FacadeCore::byte_budget(FileSystemResource::ReadBytes, maximum);
        let mut buffer = [0_u8; 8192];
        loop {
            let remaining = read_budget.remaining();
            let read_len =
                usize::try_from(remaining.saturating_add(1)).map_or(buffer.len(), |value| value.min(buffer.len()));
            let read = reader.read_async(&mut buffer[..read_len]).await.map_err(|error| {
                self.filesystem.core().enrich(
                    FsError::from_stream_io(error, FsOperation::Read, path),
                    Some(path),
                    FsOperation::Read,
                )
            })?;
            if read == 0 {
                return Ok(bytes.into_vec());
            }
            let read = FacadeCore::quantity_from_usize(
                read,
                FsOperation::Read,
                path,
                self.filesystem.properties().info().provider_id(),
            )?;
            if let Err(error) = read_budget.try_consume(read) {
                return Err(FacadeCore::budget_error(
                    error,
                    FsOperation::Read,
                    path,
                    self.filesystem.properties().info().provider_id(),
                    "read exceeds maximum byte count",
                ));
            }
            bytes
                .try_append(&buffer[..usize::try_from(read).expect("read count originated as usize")])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
        }
    }

    /// Reads at most `max_bytes` from a file asynchronously.
    ///
    /// Reading stops when the stream ends or the byte limit is reached. The
    /// returned outcome records which condition stopped the read and includes
    /// the resolved file information and original read options.
    ///
    /// # Errors
    ///
    /// Returns an error if read options cannot be resolved, the file cannot be
    /// opened or read, or the bounded read buffer cannot append data. Errors
    /// from the stream include filesystem context.
    pub(crate) async fn read_prefix(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<PrefixReadOutcome> {
        let original_options = options.clone();
        let plan = PrefixReadPlan::new(self.filesystem.properties(), path, options, max_bytes)?;
        let mut reader = self.filesystem.open_reader_resolved(path, plan.into_options()).await?;
        let info = reader.info().clone();
        if max_bytes == 0 {
            return Ok(PrefixReadOutcome::new(
                Vec::new(),
                info,
                original_options,
                max_bytes,
                PrefixReadTermination::LimitReached,
            ));
        }
        let mut bytes = ReadBuffer::new(max_bytes);
        let mut buffer = [0_u8; FacadeCore::PREFIX_BUFFER_SIZE];
        let mut termination = PrefixReadTermination::LimitReached;
        while bytes.len() < max_bytes {
            let read_len = FacadeCore::next_prefix_read_len(bytes.len(), max_bytes);
            let read = reader.read_async(&mut buffer[..read_len]).await.map_err(|error| {
                self.filesystem.core().enrich(
                    FsError::from_stream_io(error, FsOperation::Read, path),
                    Some(path),
                    FsOperation::Read,
                )
            })?;
            if read == 0 {
                termination = PrefixReadTermination::StreamEnded;
                break;
            }
            bytes
                .try_append(&buffer[..read])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
        }
        Ok(PrefixReadOutcome::new(
            bytes.into_vec(),
            info,
            original_options,
            max_bytes,
            termination,
        ))
    }
}
