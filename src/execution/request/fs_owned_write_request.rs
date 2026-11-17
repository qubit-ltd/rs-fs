// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::path::Path;
use crate::write::WriteOptions;

/// Owned parameters transferred to a managed write operation.
///
/// The byte buffer remains owned by the request until the provider accepts
/// the operation; a rejected submission returns the request to the caller.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::execution::FsOwnedWriteRequest;
/// use qubit_fs::Path;
/// use qubit_fs::write::WriteOptions;
///
/// let request = FsOwnedWriteRequest {
///     path: Path::parse("/reports/daily.csv").expect("static path is valid"),
///     contents: b"date,total\\n2026-11-16,42\\n".to_vec(),
///     options: WriteOptions::default(),
/// };
/// assert!(!request.contents.is_empty());
/// ```
pub struct FsOwnedWriteRequest {
    /// Validated provider-neutral destination path.
    pub path: Path,
    /// Complete content buffer transferred to the provider for publication.
    pub contents: Vec<u8>,
    /// Publication disposition, preconditions, atomicity, and durability.
    pub options: WriteOptions,
}
