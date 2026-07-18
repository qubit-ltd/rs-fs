// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Checksum algorithm model.

/// Checksum algorithm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChecksumAlgorithm {
    /// MD5 checksum.
    Md5,
    /// SHA-1 checksum.
    Sha1,
    /// SHA-256 checksum.
    Sha256,
    /// CRC32 checksum.
    Crc32,
    /// CRC32C checksum.
    Crc32c,
    /// Provider-specific checksum algorithm.
    Other(String),
}
