// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fluent filesystem provider registry assembly.

use qubit_spi::{
    ProviderDefinition,
    ProviderRegistryBuilder,
};

use crate::{
    FileSystemRegistry,
    FileSystemSpec,
    FsResult,
};

use super::file_system_registry::map_registration_error;

/// Fluent builder for a runtime-mutable filesystem registry.
#[derive(Default)]
pub struct FileSystemRegistryBuilder {
    /// Typed SPI builder retaining registrations until this builder is
    /// consumed.
    providers: ProviderRegistryBuilder<FileSystemSpec>,
}

impl FileSystemRegistryBuilder {
    /// Creates an empty filesystem provider builder.
    ///
    /// # Returns
    ///
    /// A builder containing no provider registrations.
    #[inline(always)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a self-described filesystem provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Self-described provider factory moved into registry
    ///   storage.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the provider and all selectors are registered.
    ///
    /// # Errors
    ///
    /// Returns [`crate::FsError`] if the provider descriptor conflicts with a
    /// prior registration.
    #[inline(always)]
    pub fn register<P>(&mut self, provider: P) -> FsResult<()>
    where
        P: ProviderDefinition<FileSystemSpec>,
    {
        self.providers
            .register(provider)
            .map_err(map_registration_error)
    }

    /// Builds a filesystem registry that remains mutable at runtime.
    ///
    /// # Returns
    ///
    /// The shared registry containing all accepted registrations.
    #[inline(always)]
    #[must_use]
    pub fn build(self) -> FileSystemRegistry {
        FileSystemRegistry::new(self.providers.build())
    }
}
