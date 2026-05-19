/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Resolved filesystem path.

use std::sync::Arc;

use crate::{
    FileSystem,
    FsPath,
};

/// Resolved filesystem instance and provider-local path.
#[derive(Debug)]
pub struct ResolvedPath {
    /// Filesystem instance selected from the URI.
    pub filesystem: Arc<dyn FileSystem>,
    /// Provider-local resource path.
    pub path: FsPath,
}
