// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous read operation implementation.

use std::future::poll_fn;
use std::task::Poll;

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
use crate::read::ReadAllOptions;
use crate::read::ReadConsistencyPolicy;
use crate::read::ReadOptions;
use crate::read::internal::ReadBuffer;
use crate::read::prefix_read_plan::PrefixReadPlan;

/// Yields after a bounded number of successful aggregate reads.
///
/// When the budget is exhausted, this helper wakes the current task and
/// returns `Pending` once before allowing the next provider read. The budget
/// is local to one aggregate operation and is reset when that operation is
/// polled again.
///
/// # Parameters
///
/// - `reads_since_yield`: Number of successful reads since the last yield.
async fn yield_read_budget(reads_since_yield: &mut usize) {
    if *reads_since_yield < 64 {
        return;
    }
    let mut yielded = false;
    poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
    *reads_since_yield = 0;
}

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
    ///
    /// # Parameters
    /// - `filesystem`: Facade borrowed for opening and contextualizing reads.
    ///
    /// # Returns
    /// An operation object borrowing the supplied filesystem.
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
    /// # Parameters
    /// - `path`: File to read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum aggregate byte count accepted.
    ///
    /// # Returns
    /// The complete file contents when they fit within `max_bytes`.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened or read, if its content
    /// exceeds `max_bytes`, or if the bounded read buffer cannot append data.
    ///
    /// # Panics
    /// Panics only if converting the read count back to `usize` fails, which
    /// would violate the invariant that the count originated from a `usize`.
    pub(crate) async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        // Reject a limit without a representable overflow sentinel before
        // opening the provider object, matching the synchronous read path.
        ReadAllOptions::new(max_bytes, ReadConsistencyPolicy::BestEffort)
            .sentinel_capacity()
            .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
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
        let mut reads_since_yield = 0_usize;
        loop {
            yield_read_budget(&mut reads_since_yield).await;
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
            reads_since_yield += 1;
        }
    }

    /// Reads at most `max_bytes` from a file asynchronously.
    ///
    /// Reading stops when the stream ends or the byte limit is reached. The
    /// returned outcome records which condition stopped the read and includes
    /// the resolved file information and original read options.
    ///
    /// # Parameters
    /// - `path`: File whose prefix is read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum number of prefix bytes to accumulate.
    ///
    /// # Returns
    /// The bytes read together with the termination reason and open-time
    /// metadata.
    ///
    /// # Errors
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
        let mut reads_since_yield = 0_usize;
        while bytes.len() < max_bytes {
            yield_read_budget(&mut reads_since_yield).await;
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
            reads_since_yield += 1;
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
