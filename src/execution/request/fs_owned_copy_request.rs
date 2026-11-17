// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::copy::CopyOptions;
use crate::path::Path;

/// Owned parameters transferred to a managed copy operation.
///
/// The provider consumes this request when it accepts the operation; if
/// preparation or submission fails, the caller receives the request back.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::execution::FsOwnedCopyRequest;
/// use qubit_fs::Path;
///
/// let request = FsOwnedCopyRequest {
///     source: Path::parse("/source.txt").expect("static path is valid"),
///     destination: Path::parse("/archive/source.txt").expect("static path is valid"),
///     options: CopyOptions::file(),
/// };
/// assert_ne!(request.source, request.destination);
/// ```
pub struct FsOwnedCopyRequest {
    /// Validated path of the resource to copy.
    pub source: Path,
    /// Validated path where the copied resource is published.
    pub destination: Path,
    /// File/tree mode, conflict handling, and publication requirements.
    pub options: CopyOptions,
}
