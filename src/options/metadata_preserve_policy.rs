/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Metadata preservation policy.

/// Metadata preservation policy for copy operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataPreservePolicy {
    /// Do not preserve metadata.
    None,
    /// Preserve portable metadata fields.
    Portable,
    /// Preserve user-defined metadata.
    UserMetadata,
    /// Preserve provider-native metadata when possible.
    ProviderNative,
    /// Preserve every metadata field that the provider can represent.
    All,
}

impl Default for MetadataPreservePolicy {
    /// Preserves portable metadata by default.
    #[inline]
    fn default() -> Self {
        Self::Portable
    }
}
