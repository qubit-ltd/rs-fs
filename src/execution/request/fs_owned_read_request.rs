// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::path::Path;
use crate::read::ReadAllOptions;

/// Owned parameters transferred to a managed complete-file read.
///
/// The submitter retains ownership if preparation or submission is rejected;
/// otherwise the provider task owns the request through execution.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::execution::FsOwnedReadRequest;
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadAllOptions;
///
/// let request = FsOwnedReadRequest {
///     path: Path::parse("/reports/daily.csv").expect("static path is valid"),
///     options: ReadAllOptions::default(),
/// };
/// assert!(request.options.max_bytes > 0);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsOwnedReadRequest {
    /// Validated provider-neutral path whose contents are requested.
    pub path: Path,
    /// Maximum content size and consistency requirement applied to the read.
    pub options: ReadAllOptions,
}
