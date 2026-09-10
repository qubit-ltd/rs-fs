// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
// qubit-style: allow source-test-pair -- public aggregate reads are tested in
// read_buffer_tests.
//! Fallible geometric buffer growth independent of provider metadata hints.

use std::collections::TryReserveError;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;

/// Accumulates bytes without exceeding the caller's logical byte ceiling.
pub(crate) struct ReadBuffer {
    /// Successfully accumulated bytes.
    bytes: Vec<u8>,
    /// Maximum logical length and requested growth target, not an RSS limit.
    maximum: usize,
}

impl ReadBuffer {
    /// Creates an empty buffer without allocating memory.
    pub(crate) const fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    /// Returns the number of successfully accumulated bytes.
    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Appends a chunk after fallibly reserving any required capacity.
    ///
    /// # Errors
    /// Returns ResourceLimitExceeded for logical overflow or allocation
    /// failure. Allocation failures retain their source; the caller adds
    /// path context.
    pub(crate) fn try_append(&mut self, chunk: &[u8]) -> FsResult<()> {
        self.try_append_with(chunk, Vec::try_reserve_exact)
    }

    /// Transfers the accumulated bytes without another allocation.
    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    /// Reserves before mutation, allowing deterministic private failure tests.
    ///
    /// `reserve` must implement Vec's additional-capacity contract. It is only
    /// called when the existing capacity cannot hold `chunk`.
    fn try_append_with(
        &mut self,
        chunk: &[u8],
        reserve: impl FnOnce(&mut Vec<u8>, usize) -> Result<(), TryReserveError>,
    ) -> FsResult<()> {
        let needed = self
            .bytes
            .len()
            .checked_add(chunk.len())
            .filter(|needed| *needed <= self.maximum)
            .ok_or_else(|| {
                FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Read,
                    "read exceeds maximum byte count",
                )
            })?;
        if needed > self.bytes.capacity() {
            let grown = self.bytes.capacity().saturating_mul(2).max(8192);
            let target = grown.max(needed).min(self.maximum);
            let additional = target - self.bytes.len();
            reserve(&mut self.bytes, additional).map_err(|error| {
                FsError::with_source(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Read,
                    "read buffer allocation exceeds available capacity",
                    error,
                )
            })?;
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::ReadBuffer;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;

    /// Initial allocation failure leaves the accumulator untouched.
    #[test]
    fn test_initial_reserve_failure_preserves_source_and_bytes() {
        let mut buffer = ReadBuffer::new(20_000);
        let error = buffer
            .try_append_with(b"payload", |_, _| Vec::<u8>::new().try_reserve_exact(usize::MAX))
            .expect_err("deterministic capacity overflow");
        assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
        assert_eq!(error.operation(), FsOperation::Read);
        assert!(error.source().is_some());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.bytes.capacity(), 0);
    }

    /// Later growth failure must preserve bytes already returned by the reader.
    #[test]
    fn test_later_reserve_failure_preserves_prior_bytes() {
        let mut buffer = ReadBuffer::new(20_000);
        buffer.try_append(b"saved").expect("initial reserve");
        let chunk = vec![9; buffer.bytes.capacity()];
        let error = buffer
            .try_append_with(&chunk, |_, _| Vec::<u8>::new().try_reserve_exact(usize::MAX))
            .expect_err("growth fails");
        assert!(error.source().is_some());
        assert_eq!(buffer.into_vec(), b"saved");
    }

    /// The requested capacity grows geometrically and stops at the ceiling.
    #[test]
    fn test_growth_is_geometric_and_requested_capacity_is_bounded() {
        let mut buffer = ReadBuffer::new(100_000);
        let mut reservations = 0;
        for _ in 0..100 {
            buffer
                .try_append_with(&[1; 1000], |bytes, additional| {
                    reservations += 1;
                    assert!(bytes.len() + additional <= 100_000);
                    bytes.try_reserve_exact(additional)
                })
                .expect("append within limit");
        }
        assert!(reservations <= 5, "growth must not reallocate for each chunk");
        assert_eq!(buffer.len(), 100_000);
        assert_eq!(
            buffer.try_append(b"x").expect_err("over limit").kind(),
            FsErrorKind::ResourceLimitExceeded
        );
    }

    /// Empty chunks never request capacity, even at a zero ceiling.
    #[test]
    fn test_empty_chunk_does_not_allocate() {
        let mut buffer = ReadBuffer::new(0);
        buffer
            .try_append_with(b"", |_, _| panic!("no allocation"))
            .expect("empty append");
        assert!(buffer.into_vec().is_empty());
    }
}
