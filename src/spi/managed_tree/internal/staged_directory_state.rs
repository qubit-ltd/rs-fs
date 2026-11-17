// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validation state for a staged directory.

use std::collections::BTreeSet;

use crate::metadata::ResourceVersion;

/// Validation progress and cleanup state shared with the staged-directory
/// owner.
pub(in crate::spi::managed_tree) struct StagedDirectoryState {
    /// Number of entries accepted by validation.
    pub(in crate::spi::managed_tree) entries: usize,
    /// Total content bytes accepted by validation.
    pub(in crate::spi::managed_tree) content_bytes: u64,
    /// Normalized relative paths used to detect duplicates.
    pub(in crate::spi::managed_tree) normalized_paths: BTreeSet<String>,
    /// Provider version observed after validation.
    pub(in crate::spi::managed_tree) validated_version: Option<ResourceVersion>,
    /// Whether publication has transferred the destination state.
    pub(in crate::spi::managed_tree) published: bool,
}
