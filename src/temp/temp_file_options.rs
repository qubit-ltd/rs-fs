/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary file creation options.

use crate::FsPath;

/// Options controlling temporary file creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TempFileOptions {
    /// Parent directory or prefix for the temporary file.
    pub parent: Option<FsPath>,
    /// Name prefix.
    pub prefix: String,
    /// Name suffix.
    pub suffix: String,
}

impl Default for TempFileOptions {
    #[inline]
    fn default() -> Self {
        Self {
            parent: None,
            prefix: ".tmp-".to_owned(),
            suffix: String::new(),
        }
    }
}
