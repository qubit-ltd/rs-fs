// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Content checksum metadata.

use crate::metadata::ChecksumAlgorithm;

/// Associates a checksum value with the algorithm that produced it.
///
/// The value is stored as text so callers can preserve the encoding supplied
/// by a filesystem provider; this type does not validate the value's format.
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
    /// Algorithm the provider used to compute `value`.
    pub algorithm: ChecksumAlgorithm,
    /// Provider-encoded checksum text, retained without format validation.
    pub value: String,
}

impl Checksum {
    /// Creates checksum metadata from an algorithm and its encoded value.
    ///
    /// The value is copied into owned storage and is not checked against the
    /// selected algorithm.
    ///
    /// # Parameters
    /// - `algorithm`: Algorithm that produced the checksum.
    /// - `value`: Provider-encoded checksum text to retain.
    ///
    /// # Returns
    /// Checksum metadata containing the supplied algorithm and value.
    #[inline]
    #[must_use]
    pub fn new(algorithm: ChecksumAlgorithm, value: &str) -> Self {
        Self {
            algorithm,
            value: value.to_owned(),
        }
    }
}
