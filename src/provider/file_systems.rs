// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Global filesystem registry facade.

use std::sync::{
    Arc,
    OnceLock,
};

use parking_lot::RwLock;
use qubit_spi::ServiceProvider;

use crate::{
    FileResource,
    FileSystem,
    FileSystemProvider,
    FileSystemRegistry,
    FileSystemSpec,
    FsResult,
    FsUri,
};

static GLOBAL_REGISTRY: OnceLock<RwLock<FileSystemRegistry>> = OnceLock::new();

/// Global filesystem registry facade.
///
/// `FileSystems` is an uninhabited namespace type. It cannot be instantiated
/// and only exposes static methods backed by a process-wide registry.
pub enum FileSystems {}

impl FileSystems {
    fn registry() -> &'static RwLock<FileSystemRegistry> {
        GLOBAL_REGISTRY.get_or_init(|| RwLock::new(FileSystemRegistry::new()))
    }

    /// Registers a filesystem provider in the global registry.
    ///
    /// # Parameters
    /// - `provider`: Filesystem provider to register.
    ///
    /// # Errors
    /// Returns an error when a provider with the same descriptor already
    /// exists.
    pub fn register<P>(provider: P) -> FsResult<()>
    where
        P: ServiceProvider<FileSystemSpec> + 'static,
    {
        let mut registry = Self::registry().write();
        registry.register(provider)
    }

    /// Registers a shared filesystem provider in the global registry.
    ///
    /// # Parameters
    /// - `provider`: Shared filesystem provider to register.
    ///
    /// # Errors
    /// Returns an error when a provider with the same descriptor already
    /// exists.
    pub fn register_shared(provider: Arc<FileSystemProvider>) -> FsResult<()> {
        let mut registry = Self::registry().write();
        registry.register_shared(provider)
    }

    /// Resolves a URI into a filesystem instance from the global registry.
    ///
    /// # Parameters
    /// - `uri`: Filesystem URI.
    ///
    /// # Returns
    /// A filesystem instance created by the matching provider.
    ///
    /// # Errors
    /// Returns an error when the URI cannot be parsed or no provider can create
    /// a filesystem for the URI scheme.
    pub fn fs(uri: &str) -> FsResult<Arc<dyn FileSystem>> {
        let uri = FsUri::parse(uri)?;
        Self::fs_for_uri(&uri)
    }

    /// Resolves a parsed URI into a filesystem instance from the global
    /// registry.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// A filesystem instance created by the matching provider.
    ///
    /// # Errors
    /// Returns an error when no provider can create a filesystem for the URI
    /// scheme.
    pub fn fs_for_uri(uri: &FsUri) -> FsResult<Arc<dyn FileSystem>> {
        let registry = Self::registry().read();
        registry.fs(uri)
    }

    /// Resolves a filesystem instance from a URI scheme.
    ///
    /// This is equivalent to resolving the minimal URI `{scheme}:///`. It is
    /// only suitable for providers that can be created from default authority,
    /// root path, and default options.
    ///
    /// # Parameters
    /// - `scheme`: URI scheme used to select a provider.
    ///
    /// # Returns
    /// A filesystem instance created by the matching provider.
    ///
    /// # Errors
    /// Returns an error when the scheme cannot form a valid URI or no provider
    /// can create a filesystem from the minimal URI.
    pub fn fs_for_scheme(scheme: &str) -> FsResult<Arc<dyn FileSystem>> {
        let uri = FsUri::parse(&format!("{scheme}:///"))?;
        Self::fs_for_uri(&uri)
    }

    /// Resolves a URI into a bound file resource from the global registry.
    ///
    /// # Parameters
    /// - `uri`: Filesystem URI.
    ///
    /// # Returns
    /// A file resource containing the matching filesystem and filesystem-local
    /// path.
    ///
    /// # Errors
    /// Returns an error when the URI cannot be parsed or no provider can create
    /// a filesystem for the URI scheme.
    pub fn resource(uri: &str) -> FsResult<FileResource> {
        let uri = FsUri::parse(uri)?;
        Self::resource_for_uri(&uri)
    }

    /// Resolves a parsed URI into a bound file resource from the global
    /// registry.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// A file resource containing the matching filesystem and filesystem-local
    /// path.
    ///
    /// # Errors
    /// Returns an error when no provider can create a filesystem for the URI
    /// scheme.
    pub fn resource_for_uri(uri: &FsUri) -> FsResult<FileResource> {
        let registry = Self::registry().read();
        registry.resource(uri)
    }

    /// Lists provider names registered in the global registry.
    ///
    /// # Returns
    /// Registered provider names.
    #[must_use]
    pub fn provider_names() -> Vec<String> {
        let registry = Self::registry().read();
        registry
            .provider_names()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}
