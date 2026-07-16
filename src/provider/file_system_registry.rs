// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem provider registry.

use std::sync::Arc;

use qubit_spi::error::ResolutionError;
use qubit_spi::{
    FallbackPolicy,
    ProviderRegistry,
    ProviderResolver,
};

use crate::{
    FileResource,
    FileSystem,
    FileSystemConfig,
    FileSystemRegistryBuilder,
    FileSystemSpec,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    FsUri,
};

/// Registry of filesystem providers.
pub struct FileSystemRegistry {
    /// Resolver applying the filesystem fallback policy.
    resolver: ProviderResolver<FileSystemSpec>,
}

impl FileSystemRegistry {
    /// Creates a filesystem registry from explicitly assembled providers.
    ///
    /// # Returns
    /// Immutable registry that resolves one provider by the URI scheme.
    #[inline]
    #[must_use]
    pub fn new(providers: ProviderRegistry<FileSystemSpec>) -> Self {
        let resolver =
            ProviderResolver::new(providers, FallbackPolicy::OnAbsence);
        Self { resolver }
    }

    /// Creates a mutable builder for startup-only filesystem-provider assembly.
    #[must_use]
    pub fn builder() -> FileSystemRegistryBuilder {
        FileSystemRegistryBuilder::new()
    }

    /// Resolves a parsed URI into a filesystem instance.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// Shared filesystem instance created by the selected provider.
    ///
    /// # Errors
    /// Returns [`FsError`] when provider resolution or creation fails.
    pub fn fs(&self, uri: &FsUri) -> FsResult<Arc<dyn FileSystem>> {
        let config = FileSystemConfig::new(uri.clone());
        self.resolver
            .create_named(uri.scheme.as_str(), &config)
            .map(|created| created.into_service())
            .map_err(map_resolution_error)
    }

    /// Resolves a parsed URI into a bound file resource.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// A file resource containing the matching filesystem and filesystem-local
    /// path.
    ///
    /// # Errors
    /// Returns [`FsError`] when provider resolution or creation fails.
    pub fn resource(&self, uri: &FsUri) -> FsResult<FileResource> {
        let path = uri.path.clone();
        let fs = self.fs(uri)?;
        Ok(FileResource::new(fs, path))
    }

    /// Gets registered provider IDs in registration order.
    ///
    /// # Returns
    /// Canonical provider IDs.
    #[inline]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<&str> {
        self.resolver
            .registry()
            .provider_ids()
            .map(|id| id.as_str())
            .collect()
    }
}

/// Maps SPI resolution errors into filesystem errors.
fn map_resolution_error(error: ResolutionError) -> FsError {
    let kind = if error.is_absence() {
        FsErrorKind::ProviderUnavailable
    } else {
        FsErrorKind::Other
    };
    let message = error.to_string();
    FsError::with_source(kind, FsOperation::Provider, &message, error)
}
