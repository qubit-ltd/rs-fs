// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Typed filesystem capability identifiers.

/// A stable operation or semantic guarantee advertised by a filesystem.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum FileSystemCapability {
    /// Directory or prefix listing.
    List = 0,
    /// Sequential byte reads.
    Read = 1,
    /// Required byte-range reads.
    RangeRead = 2,
    /// Version-conditional reads.
    ConditionalRead = 3,
    /// Provider-backed checksum validation for reads.
    ChecksumValidation = 4,
    /// File or object writes.
    Write = 5,
    /// Append writes.
    Append = 6,
    /// Conditional writes.
    ConditionalWrite = 7,
    /// Directory creation.
    CreateDirectory = 8,
    /// Native empty-directory representation.
    EmptyDirectory = 9,
    /// Resource deletion.
    Delete = 10,
    /// Recursive directory or prefix deletion.
    RecursiveDelete = 11,
    /// Version-conditional deletion.
    ConditionalDelete = 12,
    /// Rename or move.
    Rename = 13,
    /// Atomic rename.
    AtomicRename = 14,
    /// Atomic replacement publication.
    AtomicReplace = 15,
    /// Same-filesystem copy.
    Copy = 16,
    /// Server-side copy without downloading payload bytes.
    ServerSideCopy = 17,
    /// Symbolic links.
    Symlink = 18,
    /// Native or configured temporary files.
    TempFile = 19,
    /// Native or configured temporary directories.
    TempDirectory = 20,
    /// Atomic temporary-resource persistence.
    AtomicTempPersist = 21,
    /// Copy completion with explicit storage durability confirmation.
    DurableCopy = 22,
}

impl FileSystemCapability {
    /// Stable list of every capability known by this crate version.
    pub(crate) const ALL: [Self; 23] = [
        Self::List,
        Self::Read,
        Self::RangeRead,
        Self::ConditionalRead,
        Self::ChecksumValidation,
        Self::Write,
        Self::Append,
        Self::ConditionalWrite,
        Self::CreateDirectory,
        Self::EmptyDirectory,
        Self::Delete,
        Self::RecursiveDelete,
        Self::ConditionalDelete,
        Self::Rename,
        Self::AtomicRename,
        Self::AtomicReplace,
        Self::Copy,
        Self::ServerSideCopy,
        Self::Symlink,
        Self::TempFile,
        Self::TempDirectory,
        Self::AtomicTempPersist,
        Self::DurableCopy,
    ];

    /// Returns the bit representing this capability in a capability set.
    #[inline(always)]
    pub(crate) const fn bit(self) -> u128 {
        1_u128 << (self as u8)
    }
}
