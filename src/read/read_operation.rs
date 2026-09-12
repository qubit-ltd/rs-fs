// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Synchronous read operation implementation.

use crate::FileSystem;
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
use crate::read::internal::read_retry_interrupted;
use crate::read::prefix_read_plan::PrefixReadPlan;

/// Executes aggregate synchronous read operations for one facade.
pub(crate) struct ReadOperation<'a> {
    /// Facade that opens and contextualizes the reader.
    filesystem: &'a FileSystem,
}

impl<'a> ReadOperation<'a> {
    /// Creates a read operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a FileSystem) -> Self {
        Self { filesystem }
    }

    /// Reads one file into memory up to `max_bytes` after opening a reader.
    pub(crate) fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        let mut reader = self.filesystem.open_reader(path, options.clone())?;
        let mut result = ReadBuffer::new(max_bytes);
        let maximum = FacadeCore::quantity_from_usize(
            max_bytes,
            FsOperation::Read,
            path,
            self.filesystem.properties().info().provider_id(),
        )?;
        let mut read_budget = FacadeCore::byte_budget(FileSystemResource::ReadBytes, maximum);
        if let Some(metadata) = reader.info().metadata()
            && let Some(length) = metadata.len()
        {
            let selected = options.selected_length(length);
            read_budget.check_available(selected).map_err(|error| {
                FacadeCore::budget_error(
                    error,
                    FsOperation::Read,
                    path,
                    self.filesystem.properties().info().provider_id(),
                    "read exceeds maximum byte count",
                )
            })?;
        }
        let mut buffer = [0_u8; 8192];
        loop {
            let remaining = read_budget.remaining();
            let read_len =
                usize::try_from(remaining.saturating_add(1)).map_or(buffer.len(), |value| value.min(buffer.len()));
            let read = read_retry_interrupted(&mut reader, &mut buffer[..read_len], || Ok(())).map_err(|error| {
                FsError::from_stream_io(error, FsOperation::Read, path)
                    .with_provider(self.filesystem.properties().info().provider_id())
            })?;
            if read == 0 {
                return Ok(result.into_vec());
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
            result
                .try_append(&buffer[..usize::try_from(read).expect("read count originated as usize")])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
        }
    }

    /// Reads at most `max_bytes` from a file without requiring a complete read.
    pub(crate) fn read_prefix(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<PrefixReadOutcome> {
        let original_options = options.clone();
        let plan = PrefixReadPlan::new(self.filesystem.properties(), path, options, max_bytes)?;
        let mut reader = self.filesystem.open_reader_resolved(path, plan.into_options())?;
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
        let mut result = ReadBuffer::new(max_bytes);
        let mut buffer = [0_u8; FacadeCore::PREFIX_BUFFER_SIZE];
        let mut termination = PrefixReadTermination::LimitReached;
        while result.len() < max_bytes {
            let read_len = FacadeCore::next_prefix_read_len(result.len(), max_bytes);
            let read = read_retry_interrupted(&mut reader, &mut buffer[..read_len], || Ok(())).map_err(|error| {
                FsError::from_stream_io(error, FsOperation::Read, path)
                    .with_provider(self.filesystem.properties().info().provider_id())
            })?;
            if read == 0 {
                termination = PrefixReadTermination::StreamEnded;
                break;
            }
            result
                .try_append(&buffer[..read])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
        }
        Ok(PrefixReadOutcome::new(
            result.into_vec(),
            info,
            original_options,
            max_bytes,
            termination,
        ))
    }
}
