// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounds and consistency requirements for complete-file reads.
use super::ReadConsistencyPolicy;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
/// Options for a bounded complete-file read.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::read::{ReadAllOptions, ReadConsistencyPolicy};
///
/// let options = ReadAllOptions::new(1_024, ReadConsistencyPolicy::BestEffort);
/// assert_eq!(1_025, options.sentinel_capacity()?);
/// assert_eq!("sample", ReadAllOptions::decode_text(b"sample".to_vec())?);
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadAllOptions {
    /// Maximum returned bytes; defaults to 16 MiB.
    pub max_bytes: usize,
    /// Consistency requirement for the opened object.
    pub consistency: ReadConsistencyPolicy,
}
impl ReadAllOptions {
    /// Creates explicit read bounds and consistency policy.
    ///
    /// # Parameters
    /// - `max_bytes`: Maximum content length accepted for the complete read.
    /// - `consistency`: Consistency requirement for the opened object.
    ///
    /// # Returns
    /// Options with the requested content limit and consistency requirement.
    #[inline]
    #[must_use]
    pub const fn new(max_bytes: usize, consistency: ReadConsistencyPolicy) -> Self {
        Self { max_bytes, consistency }
    }

    /// Returns the allocation ceiling needed to detect one byte beyond this
    /// request's content limit.
    ///
    /// The extra byte is a sentinel: a read that fills exactly `max_bytes`
    /// must continue until EOF or until it observes that sentinel. This check
    /// is intended to run before opening or reading the provider object.
    ///
    /// # Errors
    /// Returns `ResourceLimitExceeded` when `max_bytes + 1` cannot be
    /// represented by this platform's `usize`.
    ///
    /// # Returns
    /// The content limit plus one sentinel byte for detecting overflow.
    #[inline]
    pub fn sentinel_capacity(self) -> FsResult<usize> {
        self.max_bytes.checked_add(1).ok_or_else(|| {
            FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "read limit cannot represent its overflow sentinel",
            )
        })
    }

    /// Decodes bounded read bytes as strict UTF-8 text.
    ///
    /// Invalid sequences are returned as a typed filesystem error retaining
    /// the original UTF-8 decoding error as its source.
    ///
    /// # Parameters
    /// - `bytes`: Complete read bytes to validate and decode as UTF-8.
    ///
    /// # Errors
    /// Returns `DataCorruption` when the bytes are not valid UTF-8.
    ///
    /// # Returns
    /// The decoded UTF-8 text, or a data-corruption error retaining the source
    /// decoding error.
    pub fn decode_text(bytes: Vec<u8>) -> FsResult<String> {
        String::from_utf8(bytes).map_err(|error| {
            FsError::with_source(
                FsErrorKind::DataCorruption,
                FsOperation::Read,
                "read content is not valid UTF-8",
                error,
            )
        })
    }
}
impl Default for ReadAllOptions {
    /// Uses a 16 MiB content limit and best-effort consistency.
    ///
    /// # Returns
    /// Default bounded-read options with a 16 MiB limit.
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            consistency: ReadConsistencyPolicy::BestEffort,
        }
    }
}
