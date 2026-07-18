// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Asynchronous filesystem provider contract.

use qubit_spi::ProviderDescriptor;

use crate::{
    AsyncFileSystem,
    FileSystemConfig,
    FileSystemResolution,
    FsFuture,
};

/// Creates asynchronous filesystems from complete provider configuration.
///
/// This contract is intentionally independent from the synchronous provider
/// SPI: a backend may support asynchronous filesystem access without also
/// implementing [`crate::FileSystem`].
pub trait AsyncFileSystemProvider: Send + Sync {
    /// Returns immutable provider identity, aliases, and selection priority.
    ///
    /// # Returns
    ///
    /// A descriptor snapshot used for atomic registry registration.
    fn descriptor(&self) -> ProviderDescriptor;

    /// Asynchronously creates and resolves a configured filesystem.
    ///
    /// The complete configuration includes the raw URI representation,
    /// explicit provider selection, non-sensitive options, and an optional
    /// credential reference. The provider owns URI-to-path decoding.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete validated provider configuration.
    ///
    /// # Returns
    ///
    /// A future resolving to an asynchronous filesystem, provider-local path,
    /// and safe canonical URI.
    fn create_configured_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>>;
}
