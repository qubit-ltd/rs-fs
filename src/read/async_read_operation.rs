// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Asynchronous read operation implementation.

use qubit_io::AsyncInput;

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::facade::facade_core::FileSystemResource;
use crate::path::Path;
use crate::read::ReadOptions;

/// Executes aggregate asynchronous read operations for one facade.
pub(crate) struct AsyncReadOperation<'a> {
    /// Facade that opens and contextualizes the reader.
    filesystem: &'a AsyncFileSystem,
}

impl<'a> AsyncReadOperation<'a> {
    /// Creates an asynchronous read operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a AsyncFileSystem) -> Self {
        Self { filesystem }
    }

    /// Reads an entire file asynchronously while enforcing `max_bytes`.
    pub(crate) async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        let mut reader = self.filesystem.open_reader(path, options.clone()).await?;
        let mut bytes = Vec::new();
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
            if let Ok(capacity) = usize::try_from(selected) {
                bytes.try_reserve(capacity).map_err(|error| {
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
            let read = reader.read_async(&mut buffer[..read_len]).await.map_err(|error| {
                self.filesystem.core().enrich(
                    FsError::from_stream_io(error, FsOperation::Read, path),
                    Some(path),
                    FsOperation::Read,
                )
            })?;
            if read == 0 {
                return Ok(bytes);
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
            bytes.extend_from_slice(&buffer[..usize::try_from(read).expect("read count originated as usize")]);
        }
    }

    /// Reads at most `max_bytes` from a file asynchronously.
    pub(crate) async fn read_prefix(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        let mut reader = self.filesystem.open_reader(path, options).await?;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::with_capacity(max_bytes.min(FacadeCore::PREFIX_BUFFER_SIZE));
        let mut buffer = [0_u8; FacadeCore::PREFIX_BUFFER_SIZE];
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
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        Ok(bytes)
    }
}
