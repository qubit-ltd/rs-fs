// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Asynchronous directory operation implementation.

use crate::AsyncFileSystem;
use crate::directory::AsyncDirectoryStream;
use crate::directory::ListOptions;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapability;
use crate::path::Path;
use crate::spi::ListRequest;
use crate::spi::ResolvedListOptions;

/// Executes asynchronous directory operations for one facade.
pub(crate) struct AsyncDirectoryOperation<'a> {
    /// Facade that validates and dispatches the operation.
    filesystem: &'a AsyncFileSystem,
}

impl<'a> AsyncDirectoryOperation<'a> {
    /// Creates an asynchronous directory operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a AsyncFileSystem) -> Self {
        Self { filesystem }
    }

    /// Opens a validated asynchronous provider directory stream.
    pub(crate) async fn list(&self, path: &Path, options: ListOptions) -> FsResult<AsyncDirectoryStream> {
        self.filesystem.core().validate_path(path, FsOperation::List)?;
        options
            .validate_for(self.filesystem.properties().info().path_semantics())
            .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::List))?;
        self.filesystem
            .core()
            .require(FileSystemCapability::List, FsOperation::List, Some(path))?;
        let page_size = self
            .filesystem
            .properties()
            .limits()
            .clamp_list_page_size(options.page_size());
        let options = options.with_page_size(page_size);
        let opened = self
            .filesystem
            .spi()
            .list(ListRequest::new(
                path,
                ResolvedListOptions::new(
                    options.clone(),
                    options
                        .symlink_policy_override()
                        .unwrap_or(self.filesystem.properties().symlink_policy()),
                ),
            ))
            .await
            .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::List))?;
        Ok(opened.into_stream(
            path.clone(),
            options,
            self.filesystem.properties().info().provider_id(),
            self.filesystem.properties().info().path_semantics(),
            *self.filesystem.properties().limits(),
        ))
    }
}
