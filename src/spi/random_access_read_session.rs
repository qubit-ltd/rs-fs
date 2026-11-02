//! Provider session for offset-based reads from one opened object.
use crate::error::FsResult;

/// Reads bytes at explicit offsets from the object opened by the provider.
/// Implementations must keep using that object and may return short reads.
///
/// # Examples
/// ```
/// use qubit_fs::spi::RandomAccessReadSession;
/// use qubit_fs::{FsResult};
/// struct Bytes(Vec<u8>);
/// impl RandomAccessReadSession for Bytes {
///     fn read_at(&mut self, offset: u64, out: &mut [u8]) -> FsResult<usize> {
///         let start = offset as usize;
///         let n = out.len().min(self.0.len().saturating_sub(start));
///         out[..n].copy_from_slice(&self.0[start..start+n]);
///         Ok(n)
///     }
/// }
/// ```
pub trait RandomAccessReadSession: Send {
    /// Reads at `offset`; returns zero when this request observes EOF.
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize>;
}
