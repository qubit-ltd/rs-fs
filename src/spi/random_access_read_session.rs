// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Provider session for offset-based reads from one opened object.
use crate::error::FsResult;

/// Reads bytes at explicit offsets from the object opened by the provider.
/// Implementations must keep using that object and may return short reads.
/// Each call is independent of previous offsets. On `Err`, the output buffer
/// may contain partial bytes and must not be treated as a complete result.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::RandomAccessReadSession;
/// use qubit_fs::FsResult;
///
/// struct Bytes(Vec<u8>);
/// impl RandomAccessReadSession for Bytes {
///     fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
///         let start = usize::try_from(offset).unwrap_or(usize::MAX);
///         let Some(remaining) = self.0.get(start..) else { return Ok(0); };
///         let count = output.len().min(remaining.len());
///         output[..count].copy_from_slice(&remaining[..count]);
///         Ok(count)
///     }
/// }
/// let mut session = Bytes(b"abc".to_vec());
/// let mut sample = [0; 4];
/// assert_eq!(2, session.read_at(1, &mut sample).unwrap()); // short read
/// assert_eq!(0, session.read_at(3, &mut sample).unwrap()); // EOF
/// ```
pub trait RandomAccessReadSession: Send {
    /// Reads from `offset` into a non-empty buffer. A direct SPI call may
    /// observe EOF and return zero. The facade handles empty requests and
    /// requests at or beyond the open-time length without calling this method;
    /// those facade-level zero results do not indicate provider-observed EOF.
    ///
    /// # Errors
    /// Returns the provider's typed read error. The buffer may have been
    /// partially modified when an error is returned.
    ///
    /// # Parameters
    /// - `offset`: Starting byte offset within the opened object.
    /// - `output`: Non-empty destination buffer for the bytes read.
    ///
    /// # Returns
    /// The number of bytes read; a short count is valid and zero means this
    /// provider request observed EOF.
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize>;
}
