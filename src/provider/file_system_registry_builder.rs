// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Startup-only filesystem provider registry assembly.

use qubit_spi::error::RegistrationError;
use qubit_spi::{
    ProviderDescriptor,
    ProviderRegistryBuilder,
    ServiceProvider,
};

use crate::{
    FileSystemRegistry,
    FileSystemSpec,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};

/// Startup-only builder for an immutable filesystem registry.
#[derive(Default)]
pub struct FileSystemRegistryBuilder {
    /// Typed SPI builder retaining registrations until startup completes.
    providers: ProviderRegistryBuilder<FileSystemSpec>,
}

impl FileSystemRegistryBuilder {
    /// Creates an empty filesystem provider builder.
    ///
    /// # Returns
    ///
    /// A builder containing no provider registrations.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a filesystem provider and its external identity metadata.
    ///
    /// # Arguments
    ///
    /// * `descriptor` - Canonical provider identity, aliases, and priority.
    /// * `provider` - Provider factory moved into registry storage.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the provider and all selectors are registered.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] if the descriptor conflicts with a prior provider.
    pub fn register<P>(
        &mut self,
        descriptor: ProviderDescriptor,
        provider: P,
    ) -> FsResult<()>
    where
        P: ServiceProvider<FileSystemSpec>,
    {
        self.providers
            .register(descriptor, provider)
            .map_err(map_registration_error)
    }

    /// Builds the immutable filesystem registry used at runtime.
    ///
    /// # Returns
    ///
    /// The immutable registry containing all accepted registrations.
    #[must_use]
    pub fn build(self) -> FileSystemRegistry {
        FileSystemRegistry::new(self.providers.build())
    }
}

/// Maps an SPI registration error into the filesystem error model.
///
/// # Arguments
///
/// * `error` - SPI selector-conflict diagnostic.
///
/// # Returns
///
/// A filesystem provider error preserving the original source.
fn map_registration_error(error: RegistrationError) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::InvalidPath,
        FsOperation::Provider,
        &message,
        error,
    )
}
