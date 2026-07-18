// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed filesystem capability identifiers.

/// A stable operation or semantic guarantee advertised by a filesystem.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum FileSystemCapability {
    /// Metadata lookup.
    Stat,
    /// Directory or prefix listing.
    List,
    /// Sequential byte reads.
    Read,
    /// Required byte-range reads.
    RangeRead,
    /// Version-conditional reads.
    ConditionalRead,
    /// Provider-backed checksum validation for reads.
    ChecksumValidation,
    /// File or object writes.
    Write,
    /// Append writes.
    Append,
    /// Conditional writes.
    ConditionalWrite,
    /// Directory creation.
    CreateDirectory,
    /// Native empty-directory representation.
    EmptyDirectory,
    /// Resource deletion.
    Delete,
    /// Recursive directory or prefix deletion.
    RecursiveDelete,
    /// Version-conditional deletion.
    ConditionalDelete,
    /// Rename or move.
    Rename,
    /// Atomic rename.
    AtomicRename,
    /// Atomic replacement publication.
    AtomicReplace,
    /// Same-filesystem copy.
    Copy,
    /// Server-side copy without downloading payload bytes.
    ServerSideCopy,
    /// Symbolic links.
    Symlink,
    /// Native or configured temporary files.
    TempFile,
    /// Native or configured temporary directories.
    TempDirectory,
    /// Atomic temporary-resource persistence.
    AtomicTempPersist,
}

impl FileSystemCapability {
    /// Returns the bit representing this capability in a capability set.
    #[inline]
    pub(crate) const fn bit(self) -> u128 {
        1_u128 << (self as u8)
    }
}
