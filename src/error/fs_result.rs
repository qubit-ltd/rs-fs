// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem result type alias.

use super::fs_error::FsError;

/// Result type used by filesystem APIs.
///
/// # Examples
///
/// ```
/// use qubit_fs::Path;
/// use qubit_fs::error::FsResult;
///
/// fn load_root() -> FsResult<Path> {
///     Path::parse("/")
/// }
/// assert!(load_root().is_ok());
/// ```
pub type FsResult<T> = Result<T, FsError>;
