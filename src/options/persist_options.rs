// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource persistence options.

use crate::{
    AtomicityRequirement,
    MetadataPreservePolicy,
};

/// Options controlling temporary resource persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistOptions {
    /// Whether the destination may be overwritten.
    pub overwrite: bool,
    /// Required atomicity level.
    pub atomic: AtomicityRequirement,
    /// Whether copy plus delete may be used when rename is unavailable.
    pub allow_copy_delete: bool,
    /// Metadata preservation policy.
    pub preserve_metadata: MetadataPreservePolicy,
}

impl Default for PersistOptions {
    #[inline]
    fn default() -> Self {
        Self {
            overwrite: false,
            atomic: AtomicityRequirement::Required,
            allow_copy_delete: false,
            preserve_metadata: MetadataPreservePolicy::Portable,
        }
    }
}
