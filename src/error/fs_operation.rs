// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Filesystem operation identifiers used in errors.

/// Filesystem operation that produced an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsOperation {
    /// Path parsing or normalization.
    ParsePath,
    /// URI parsing or normalization.
    ParseUri,
    /// Construction-time filesystem property validation.
    ValidateProperties,
    /// Verification of a provider operation outcome.
    ValidateProviderOutcome,
    /// Metadata lookup.
    Stat,
    /// Existence check.
    Exists,
    /// Directory listing.
    List,
    /// Reader creation.
    OpenReader,
    /// Byte transfer from an already-open reader.
    Read,
    /// Writer creation.
    OpenWriter,
    /// Byte transfer to an already-open writer.
    Write,
    /// Writer publication.
    CommitWriter,
    /// Writer cancellation and cleanup.
    AbortWriter,
    /// Directory creation.
    CreateDir,
    /// File or directory deletion.
    Delete,
    /// Rename or move.
    Rename,
    /// Copy.
    Copy,
    /// Provider copy primitive invocation.
    BeginCopy,
    /// Temporary resource creation.
    CreateTemp,
    /// Temporary resource cleanup.
    CleanupTemp,
    /// Temporary-resource ownership transfer to the caller.
    KeepTemp,
    /// Temporary resource persistence.
    PersistTemp,
    /// Provider registration or service creation.
    Provider,
    /// Operation not covered by a more specific variant.
    Other,
}
