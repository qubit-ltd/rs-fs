// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Content checksum metadata.

use crate::metadata::ChecksumAlgorithm;

/// Content checksum.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{Checksum, ChecksumAlgorithm};
///
/// let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "deadbeef");
/// assert_eq!(ChecksumAlgorithm::Sha256, checksum.algorithm);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checksum {
    /// Algorithm used to compute the checksum.
    pub algorithm: ChecksumAlgorithm,
    /// Encoded checksum value.
    pub value: String,
}

impl Checksum {
    /// Creates a checksum.
    ///
    /// # Parameters
    /// - `algorithm`: Checksum algorithm.
    /// - `value`: Encoded checksum value.
    ///
    /// # Returns
    /// New checksum.
    #[inline]
    #[must_use]
    pub fn new(algorithm: ChecksumAlgorithm, value: &str) -> Self {
        Self {
            algorithm,
            value: value.to_owned(),
        }
    }
}
