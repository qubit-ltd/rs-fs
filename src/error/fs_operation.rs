// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem operation identifiers used in errors.

/// Filesystem operation that produced an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsOperation {
    /// Path parsing or normalization.
    ParsePath,
    /// URI parsing or normalization.
    ParseUri,
    /// Metadata lookup.
    Metadata,
    /// Existence check.
    Exists,
    /// Directory listing.
    List,
    /// Reader creation.
    OpenReader,
    /// Writer creation.
    OpenWriter,
    /// Directory creation.
    CreateDir,
    /// File or directory deletion.
    Delete,
    /// Rename or move.
    Rename,
    /// Copy.
    Copy,
    /// Temporary resource creation.
    CreateTemp,
    /// Temporary resource cleanup.
    CleanupTemp,
    /// Temporary resource persistence.
    PersistTemp,
    /// Provider registration or service creation.
    Provider,
    /// Operation not covered by a more specific variant.
    Other,
}
