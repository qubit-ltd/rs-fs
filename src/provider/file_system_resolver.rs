/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Filesystem URI resolver.

use std::sync::Arc;

use crate::{
    FileSystemRegistry,
    FsResult,
    FsUri,
    ResolvedPath,
};

/// URI resolver backed by a filesystem provider registry.
#[derive(Debug)]
pub struct FileSystemResolver {
    /// Provider registry.
    registry: Arc<FileSystemRegistry>,
}

impl FileSystemResolver {
    /// Creates a resolver.
    ///
    /// # Parameters
    /// - `registry`: Provider registry.
    ///
    /// # Returns
    /// Resolver using the supplied registry.
    #[inline]
    #[must_use]
    pub fn new(registry: Arc<FileSystemRegistry>) -> Self {
        Self { registry }
    }

    /// Resolves a URI string.
    ///
    /// # Parameters
    /// - `uri`: Filesystem URI.
    ///
    /// # Returns
    /// Filesystem instance and provider-local path.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when parsing or provider creation fails.
    pub fn resolve(&self, uri: &str) -> FsResult<ResolvedPath> {
        let uri = FsUri::parse(uri)?;
        self.resolve_uri(uri)
    }

    /// Resolves a parsed URI.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// Filesystem instance and provider-local path.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when provider creation fails.
    pub fn resolve_uri(&self, uri: FsUri) -> FsResult<ResolvedPath> {
        let path = uri.path.clone();
        let filesystem = self.registry.open_uri(uri)?;
        Ok(ResolvedPath { filesystem, path })
    }
}
