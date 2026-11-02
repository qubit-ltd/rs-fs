//! Application-facing asynchronous handle for bounded offset reads.
use crate::error::{FsError, FsErrorKind, FsOperation, FsResult};
use crate::metadata::{FileSystemLimit, OpenedFileInfo};
use crate::read::RandomAccessReadPolicy;
use crate::spi::AsyncRandomAccessReadSession;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::pin::Pin;

/// An async random-access session bound to one opened object.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::{AsyncFileSystem, Path};
/// use qubit_fs::read::ReadOptions;
///
/// async fn sample(filesystem: &AsyncFileSystem, path: &Path) -> Result<Vec<u8>, qubit_fs::FsError> {
///     let mut reader = filesystem.open_random_access_reader(path, ReadOptions::default()).await?;
///     let mut bytes = [0; 16];
///     let mut filled = 0;
///     while filled < bytes.len() {
///         match reader.read_at(filled as u64, &mut bytes[filled..]).await? {
///             0 => break,
///             count => filled += count,
///         }
///     }
///     Ok(bytes[..filled].to_vec())
/// }
/// ```
///
/// A short read is allowed. If a call returns `Err` or its future is dropped,
/// the output may contain partial bytes; discard the incomplete sample. The
/// cached length is not a content snapshot.
pub struct AsyncRandomAccessReader {
    info: OpenedFileInfo,
    len: u64,
    limit: FileSystemLimit,
    provider_id: String,
    session: Pin<Box<dyn AsyncRandomAccessReadSession>>,
}
impl AsyncRandomAccessReader {
    pub(crate) fn new(
        info: OpenedFileInfo,
        len: u64,
        limit: FileSystemLimit,
        provider_id: String,
        session: Box<dyn AsyncRandomAccessReadSession>,
    ) -> Self {
        Self {
            info,
            len,
            limit,
            provider_id,
            session: Box::into_pin(session),
        }
    }
    /// Returns open-time identity and metadata.
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    /// Returns the accessible upper bound observed when opened.
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Returns whether the open-time length is zero.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Reads into caller storage at an explicit offset; cancellation does not change session identity.
    ///
    /// # Parameters
    /// - `offset`: Byte offset from the beginning of the opened object.
    /// - `output`: Caller-owned destination buffer borrowed until the future
    ///   completes or is dropped.
    ///
    /// # Returns
    /// The number of bytes initialized, possibly less than the buffer length;
    /// zero means this request observed EOF.
    ///
    /// # Errors
    /// Returns `InvalidOptions` for an overflowing offset range,
    /// `ResourceLimitExceeded` when the clipped request exceeds the provider
    /// limit, a provider error with path/provider/source context, or
    /// `ProviderContractViolation` when the provider over-reports bytes. The
    /// buffer may contain partial bytes after an error or cancellation and
    /// must be discarded.
    pub async fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let count =
            RandomAccessReadPolicy::validate_read(offset, output.len(), self.len, self.limit)
                .map_err(|e| {
                    e.with_path(self.info.path().clone())
                        .with_provider(&self.provider_id)
                })?;
        if count == 0 {
            return Ok(0);
        }
        let n = self
            .session
            .as_mut()
            .read_at_async(offset, &mut output[..count])
            .await
            .map_err(|e| {
                e.with_path(self.info.path().clone())
                    .with_provider(&self.provider_id)
                    .with_operation(FsOperation::Read)
            })?;
        if n > count {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::Read,
                "provider returned more bytes than supplied buffer",
            )
            .with_path(self.info.path().clone())
            .with_provider(&self.provider_id));
        }
        Ok(n)
    }
}
impl Debug for AsyncRandomAccessReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AsyncRandomAccessReader")
            .field("info", &self.info)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}
