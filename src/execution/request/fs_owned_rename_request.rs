// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::path::Path;
use crate::rename::RenameOptions;

/// Owned parameters transferred to a managed rename operation.
///
/// The request preserves both paths and overwrite/publication policy while
/// ownership passes through preparation to the provider task.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::execution::FsOwnedRenameRequest;
/// use qubit_fs::Path;
/// use qubit_fs::rename::RenameOptions;
///
/// let request = FsOwnedRenameRequest {
///     source: Path::parse("/draft.txt").expect("static path is valid"),
///     destination: Path::parse("/final.txt").expect("static path is valid"),
///     options: RenameOptions::default(),
/// };
/// assert!(!request.options.overwrite());
/// ```
pub struct FsOwnedRenameRequest {
    /// Validated path of the resource to move.
    pub source: Path,
    /// Validated destination path for the moved resource.
    pub destination: Path,
    /// Overwrite, atomicity, and durability requirements for the rename.
    pub options: RenameOptions,
}
