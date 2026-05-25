/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Filesystem provider registry.

use std::sync::Arc;

use qubit_spi::{
    ProviderRegistry,
    ProviderRegistryError,
    ServiceProvider,
};

use crate::{
    FileResource,
    FileSystem,
    FileSystemConfig,
    FileSystemProvider,
    FileSystemSpec,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    FsUri,
};

/// Registry of filesystem providers.
#[derive(Debug, Default)]
pub struct FileSystemRegistry {
    /// Underlying typed SPI registry.
    providers: ProviderRegistry<FileSystemSpec>,
}

impl FileSystemRegistry {
    /// Creates an empty filesystem registry.
    ///
    /// # Returns
    /// Empty registry.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an owned filesystem provider.
    ///
    /// # Parameters
    /// - `provider`: Provider to register.
    ///
    /// # Errors
    /// Returns [`FsError`] when provider metadata is invalid or conflicts with
    /// an existing provider.
    pub fn register<P>(&mut self, provider: P) -> FsResult<()>
    where
        P: ServiceProvider<FileSystemSpec> + 'static,
    {
        self.providers.register(provider).map_err(map_provider_error)
    }

    /// Registers a shared filesystem provider.
    ///
    /// # Parameters
    /// - `provider`: Shared provider to register.
    ///
    /// # Errors
    /// Returns [`FsError`] when provider metadata is invalid or conflicts with
    /// an existing provider.
    pub fn register_shared(&mut self, provider: Arc<FileSystemProvider>) -> FsResult<()> {
        self.providers.register_shared(provider).map_err(map_provider_error)
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
        let selector = uri.scheme.clone();
        let config = FileSystemConfig::new(uri.clone());
        self.providers
            .create_arc(&selector, &config)
            .map_err(map_provider_error)
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

    /// Gets registered provider names in registration order.
    ///
    /// # Returns
    /// Provider names.
    #[inline]
    #[must_use]
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.provider_names()
    }
}

/// Maps SPI registry errors into filesystem errors.
///
/// # Parameters
/// - `error`: SPI registry error.
///
/// # Returns
/// Filesystem error preserving provider diagnostic text.
fn map_provider_error(error: ProviderRegistryError) -> FsError {
    let kind = match &error {
        ProviderRegistryError::EmptyProviderName
        | ProviderRegistryError::InvalidProviderName { .. }
        | ProviderRegistryError::DuplicateProviderName { .. }
        | ProviderRegistryError::DuplicateProviderCandidate { .. } => FsErrorKind::InvalidPath,
        ProviderRegistryError::UnknownProvider { .. }
        | ProviderRegistryError::ProviderUnavailable { .. }
        | ProviderRegistryError::NoAvailableProvider { .. }
        | ProviderRegistryError::EmptyRegistry => FsErrorKind::ProviderUnavailable,
        ProviderRegistryError::ProviderCreate { .. } => FsErrorKind::Other,
    };
    let message = error.to_string();
    FsError::with_source(kind, FsOperation::Provider, &message, error)
}
