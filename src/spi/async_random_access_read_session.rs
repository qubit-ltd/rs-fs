//! Runtime-neutral provider session for asynchronous offset reads.
use super::SpiFuture;
use crate::error::FsResult;
use std::pin::Pin;

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
    /// Reads at `offset`, allowing short reads and returning zero at EOF.
    ///
    /// # Errors
    /// Resolves to the provider's typed read error. The caller's buffer may
    /// contain partial bytes after an error, and must not be used as a complete
    /// sample.
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        output: &'a mut [u8],
    ) -> SpiFuture<'a, FsResult<usize>>;
}
