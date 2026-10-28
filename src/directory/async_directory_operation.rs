// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous directory operation implementation.

use crate::AsyncFileSystem;
use crate::directory::AsyncDirectoryStream;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapability;
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

    /// Opens a provider directory stream after validating the requested scope
    /// and options.
    ///
    /// The requested page size is clamped to the filesystem limit before
    /// dispatch. The stream retains the effective symlink policy and
    /// provider path semantics for subsequent entries.
    ///
    /// # Parameters
    /// - `scope`: Validated directory path or namespace to enumerate.
    /// - `options`: Listing filters, page size, and caller-side limits.
    ///
    /// # Returns
    /// A validated asynchronous stream over the provider's directory entries.
    ///
    /// # Errors
    ///
    /// Returns an enriched filesystem error when the scope or options are
    /// invalid, listing is unsupported, the provider cannot open the listing,
    /// or the returned stream cannot be initialized.
    pub(crate) async fn list(&self, scope: &ListScope, options: ListOptions) -> FsResult<AsyncDirectoryStream> {
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
        let opened = self
            .filesystem
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
            .await
            .map_err(|error| self.filesystem.core().enrich(error, scope.path(), FsOperation::List))?;
        opened.into_stream(
            scope.clone(),
            options,
            self.filesystem.properties().info().provider_id(),
            self.filesystem.properties().info().path_semantics(),
            *self.filesystem.properties().limits(),
            self.filesystem.properties().path_constraints().clone(),
        )
    }
}
