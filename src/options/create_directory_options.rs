// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Directory creation options.

use crate::{NonSensitiveMetadata, UserMetadata};

/// Options controlling directory or collection creation.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateDirectoryOptions {
    /// Whether missing parent directories should be created.
    pub recursive: bool,
    /// Whether an existing directory should be accepted.
    pub exists_ok: bool,
    /// User-defined metadata with validated non-sensitive structural keys.
    pub user_metadata: NonSensitiveMetadata,
}

impl Default for CreateDirectoryOptions {
    /// Creates non-recursive options that reject an existing directory.
    fn default() -> Self {
        Self {
            recursive: false,
            exists_ok: false,
            user_metadata: NonSensitiveMetadata::new(),
        }
    }
}

impl CreateDirectoryOptions {
    /// Replaces user-defined metadata that has already passed key validation.
    #[must_use]
    pub fn with_user_metadata(mut self, metadata: UserMetadata) -> Self {
        self.user_metadata = NonSensitiveMetadata::from(metadata);
        self
    }
}
