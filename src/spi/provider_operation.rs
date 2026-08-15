// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- exercised through the public provider
// property contract tests.
//! Provider entry-point identifiers used for facade dispatch.

/// A concrete operation entry point implemented by a filesystem provider.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ProviderOperation {
    /// Reads metadata for one path.
    Stat = 0,
    /// Opens a directory listing session.
    List = 1,
    /// Opens a file reader.
    OpenReader = 2,
    /// Opens a file writer.
    OpenWriter = 3,
    /// Creates a directory.
    CreateDirectory = 4,
    /// Deletes a file.
    DeleteFile = 5,
    /// Deletes a directory.
    DeleteDirectory = 6,
    /// Attempts a provider-native copy.
    TryCopy = 7,
    /// Renames a resource.
    Rename = 8,
    /// Creates a temporary file.
    CreateTempFile = 9,
    /// Creates a temporary directory.
    CreateTempDirectory = 10,
}
