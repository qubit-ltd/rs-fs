//! Runtime-neutral provider session for asynchronous offset reads.
use super::SpiFuture;
use crate::error::FsResult;
use std::pin::Pin;

/// Asynchronously reads one opened object at explicit offsets.
/// Dropping a pending future ends its buffer borrow; implementations must not
/// retain or later mutate the caller's buffer after the future is dropped.
pub trait AsyncRandomAccessReadSession: Send {
    /// Reads at `offset`, allowing short reads and returning zero at EOF.
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        output: &'a mut [u8],
    ) -> SpiFuture<'a, FsResult<usize>>;
}
