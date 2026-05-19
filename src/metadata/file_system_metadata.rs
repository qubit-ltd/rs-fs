/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Filesystem instance metadata.

use qubit_metadata::Metadata;

use crate::{
    FileSystemCapabilities,
    PathSemantics,
};

/// Metadata describing one filesystem instance.
#[derive(Clone, Debug, PartialEq)]
pub struct FileSystemMetadata {
    /// Provider id that created this filesystem.
    pub provider_id: String,
    /// Schemes accepted by the provider.
    pub schemes: Vec<String>,
    /// Capability hints for this filesystem.
    pub capabilities: FileSystemCapabilities,
    /// Path semantics used by this filesystem.
    pub path_semantics: PathSemantics,
    /// Provider-native metadata.
    pub provider_metadata: Metadata,
}

impl FileSystemMetadata {
    /// Creates filesystem metadata for one provider.
    ///
    /// # Parameters
    /// - `provider_id`: Provider id.
    ///
    /// # Returns
    /// Metadata with default capabilities and path semantics.
    #[inline]
    #[must_use]
    pub fn new(provider_id: &str) -> Self {
        Self {
            provider_id: provider_id.to_owned(),
            schemes: Vec::new(),
            capabilities: FileSystemCapabilities::default(),
            path_semantics: PathSemantics::default(),
            provider_metadata: Metadata::new(),
        }
    }
}
