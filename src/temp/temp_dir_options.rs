// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Temporary directory creation options.

use crate::FsPath;

/// Options controlling temporary directory creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TempDirOptions {
    /// Parent directory or prefix for the temporary directory.
    pub parent: Option<FsPath>,
    /// Name prefix.
    pub prefix: String,
    /// Name suffix.
    pub suffix: String,
}

impl Default for TempDirOptions {
    #[inline]
    fn default() -> Self {
        Self {
            parent: None,
            prefix: ".tmp-dir-".to_owned(),
            suffix: String::new(),
        }
    }
}
