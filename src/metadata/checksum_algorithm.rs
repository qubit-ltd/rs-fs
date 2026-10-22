// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Checksum algorithm model.

/// Identifies the algorithm used to compute a content checksum.
///
/// Use the standard variants when the algorithm is known, or [`Other`] to
/// retain a provider-defined algorithm name when it is not represented here.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{Checksum, ChecksumAlgorithm};
///
/// let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "deadbeef");
/// assert_eq!(checksum.algorithm, ChecksumAlgorithm::Sha256);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChecksumAlgorithm {
    /// MD5 digest, commonly represented as a 128-bit value.
    Md5,
    /// SHA-1 digest, commonly represented as a 160-bit value.
    Sha1,
    /// SHA-256 digest, commonly represented as a 256-bit value.
    Sha256,
    /// CRC-32 checksum using a provider-selected CRC-32 variant.
    Crc32,
    /// CRC-32C checksum, using the Castagnoli polynomial.
    Crc32c,
    /// A checksum algorithm whose name is defined by the storage provider.
    Other(
        /// Provider-defined algorithm name, preserved for interoperability.
        String,
    ),
}
