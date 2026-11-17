// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use crate::FsError;
use crate::copy::CopyFailure;
use crate::rename::RenameFailure;
use crate::write::WriteAllFailure;

/// Typed failure produced by a managed filesystem operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsOperationFailureKind;
///
/// fn inspect(kind: &FsOperationFailureKind) {
///     match kind {
///         FsOperationFailureKind::Provider(_)
///         | FsOperationFailureKind::Copy(_)
///         | FsOperationFailureKind::Write(_)
///         | FsOperationFailureKind::Rename(_) => {}
///     }
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub enum FsOperationFailureKind {
    /// General provider or read failure retaining its structured filesystem
    /// error.
    Provider(
        /// Structured provider or read error.
        Arc<FsError>,
    ),
    /// Copy failure retaining progress and any typed recovery owner.
    Copy(
        /// Copy progress and any retained recovery owner.
        Box<CopyFailure>,
    ),
    /// Write failure retaining progress and publication facts.
    Write(
        /// Write progress and publication facts.
        Box<WriteAllFailure>,
    ),
    /// Rename failure retaining publication facts.
    Rename(
        /// Rename publication facts.
        Box<RenameFailure>,
    ),
}
