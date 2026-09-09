// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Synchronous directory operation implementation.

use crate::FileSystem;
use crate::directory::DirectoryStream;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapability;
use crate::spi::ListRequest;
use crate::spi::ResolvedListOptions;

/// Executes synchronous directory operations for one facade.
pub(crate) struct DirectoryOperation<'a> {
    /// Facade that validates and dispatches the operation.
    filesystem: &'a FileSystem,
}

impl<'a> DirectoryOperation<'a> {
    /// Creates a directory operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a FileSystem) -> Self {
        Self { filesystem }
    }

    /// Opens a provider directory stream after local option validation.
    pub(crate) fn list(&self, scope: &ListScope, options: ListOptions) -> FsResult<DirectoryStream> {
        crate::directory::internal::list_preflight::validate(self.filesystem.properties(), scope, &options)
            .map_err(|error| self.filesystem.core().enrich(error, scope.path(), FsOperation::List))?;
        self.filesystem
            .core()
            .require(FileSystemCapability::List, FsOperation::List, scope.path())?;
        let page_size = self
            .filesystem
            .properties()
            .limits()
            .clamp_list_page_size(options.page_size());
        let options = options.with_page_size(page_size);
        self.filesystem
            .spi()
            .list(ListRequest::new(
                scope,
                ResolvedListOptions::new(
                    options.clone(),
                    options
                        .symlink_policy_override()
                        .unwrap_or(self.filesystem.properties().symlink_policy()),
                ),
            ))
            .and_then(|opened| {
                DirectoryStream::new(
                    scope.clone(),
                    opened.into_stream(),
                    options,
                    self.filesystem.properties().info().provider_id(),
                    self.filesystem.properties().info().path_semantics(),
                    *self.filesystem.properties().limits(),
                )
            })
            .map_err(|error| self.filesystem.core().enrich(error, scope.path(), FsOperation::List))
    }
}
