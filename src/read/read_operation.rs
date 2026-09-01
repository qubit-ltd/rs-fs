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

use qubit_io::Input;

use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::facade::facade_core::FileSystemResource;
use crate::path::Path;
use crate::read::ReadOptions;

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
        let mut reader = self.filesystem.open_reader(path, options)?;
        let mut result = Vec::new();
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
            read_budget.check_available(length).map_err(|error| {
                FacadeCore::budget_error(
                    error,
                    FsOperation::Read,
                    path,
                    self.filesystem.properties().info().provider_id(),
                    "read exceeds maximum byte count",
                )
            })?;
            if let Ok(capacity) = usize::try_from(length) {
                result.try_reserve(capacity).map_err(|error| {
                    FsError::with_source(
                        FsErrorKind::ResourceLimitExceeded,
                        FsOperation::Read,
                        "read buffer allocation exceeds available capacity",
                        error,
                    )
                    .with_path(path.clone())
                    .with_provider(self.filesystem.properties().info().provider_id())
                })?;
            }
        }
        let mut buffer = [0_u8; 8192];
        loop {
            let remaining = read_budget.remaining();
            let read_len =
                usize::try_from(remaining.saturating_add(1)).map_or(buffer.len(), |value| value.min(buffer.len()));
            let read = Input::read(&mut reader, &mut buffer[..read_len]).map_err(|error| {
                FsError::from_stream_io(error, FsOperation::Read, path)
                    .with_provider(self.filesystem.properties().info().provider_id())
            })?;
            if read == 0 {
                return Ok(result);
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
            result.extend_from_slice(&buffer[..usize::try_from(read).expect("read count originated as usize")]);
        }
    }

    /// Reads at most `max_bytes` from a file without requiring a complete read.
    pub(crate) fn read_prefix(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        let mut reader = self.filesystem.open_reader(path, options)?;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut result = Vec::with_capacity(max_bytes.min(FacadeCore::PREFIX_BUFFER_SIZE));
        let mut buffer = [0_u8; FacadeCore::PREFIX_BUFFER_SIZE];
        while result.len() < max_bytes {
            let read_len = FacadeCore::next_prefix_read_len(result.len(), max_bytes);
            let read = Input::read(&mut reader, &mut buffer[..read_len]).map_err(|error| {
                FsError::from_stream_io(error, FsOperation::Read, path)
                    .with_provider(self.filesystem.properties().info().provider_id())
            })?;
            if read == 0 {
                break;
            }
            result.extend_from_slice(&buffer[..read]);
        }
        Ok(result)
    }
}
