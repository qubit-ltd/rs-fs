// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Runtime-neutral provider session for asynchronous offset reads.
use std::pin::Pin;

use super::SpiFuture;
use crate::error::FsResult;

/// Asynchronously reads one opened object at explicit offsets.
///
/// The returned future must perform no provider I/O until it is polled.
/// Dropping a pending future cancels local polling only. The provider must
/// continue binding later calls to the same object and must not retain or
/// modify `output` after the future is dropped. A pending or failed call may
/// already have modified part of the buffer, so callers must discard it unless
/// the future returned a successful byte count.
///
/// # Examples
///
/// ```
/// use qubit_fs::error::FsResult;
/// use qubit_fs::spi::{AsyncRandomAccessReadSession, SpiFuture};
/// use std::pin::Pin;
///
/// struct Empty;
/// impl AsyncRandomAccessReadSession for Empty {
///     fn read_at_async<'a>(self: Pin<&'a mut Self>, _: u64, _: &'a mut [u8]) -> SpiFuture<'a, FsResult<usize>> {
///         Box::pin(async { Ok(0) })
///     }
/// }
/// ```
pub trait AsyncRandomAccessReadSession: Send {
    /// Reads from `offset` into a non-empty buffer, allowing short reads. A
    /// direct SPI call may observe EOF and return zero. The facade handles
    /// empty requests and requests at or beyond the open-time length without
    /// calling this method; those facade-level zero results do not indicate
    /// provider-observed EOF.
    ///
    /// # Errors
    /// Resolves to the provider's typed read error. The caller's buffer may
    /// contain partial bytes after an error, and must not be used as a complete
    /// sample.
    ///
    /// # Parameters
    /// - `offset`: Starting byte offset within the opened object.
    /// - `output`: Non-empty destination buffer, borrowed for the lifetime of
    ///   the returned future.
    ///
    /// # Returns
    /// A future resolving to the number of bytes read; a short count is valid
    /// and zero means this request observed EOF.
    fn read_at_async<'a>(self: Pin<&'a mut Self>, offset: u64, output: &'a mut [u8]) -> SpiFuture<'a, FsResult<usize>>;
}
